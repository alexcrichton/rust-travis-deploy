use std::env;
use std::fs;
use std::process::{Command, Stdio};
use std::io::{Read, Write};
use std::os::unix::prelude::*;
use std::os::unix::net::UnixStream;

fn main() {
    let slug = env::var("TRAVIS_REPO_SLUG").unwrap();
    let key = env::var("GITHUB_DEPLOY_KEY").unwrap();

    match env::var("TRAVIS_BRANCH") {
        Ok(ref s) if s == "master" => {}
        _ => {
            println!("not the master branch, skipping deploy");
            return
        }
    }

    let socket = "/tmp/.github-deploy-socket";
    let _bomb = OnDrop(|| {
        drop(fs::remove_file(&socket));
    });
    let mut child = Command::new("ssh-agent")
        .arg("-D")
        .arg("-a").arg(&socket)
        .spawn()
        .unwrap();
    let _bomb = OnDrop(|| {
        println!("kill: {:?}", child.kill());
        println!("wait {:?}", child.wait());
    });
    while UnixStream::connect(&socket).is_err() {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    let mut decode = Command::new("base64")
        .arg("-d")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    decode.stdin.take().unwrap().write_all(&key.as_bytes()).unwrap();
    let mut key = Vec::new();
    decode.stdout.take().unwrap().read_to_end(&mut key).unwrap();
    decode.wait().unwrap();

    let path = "_the_key";
    fs::write(&path, key).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    run(Command::new("ssh-add")
        .arg(&path)
        .env("SSH_AUTH_SOCK", &socket));
    fs::remove_file(&path).unwrap();

    let sha = env::var("TRAVIS_COMMIT").unwrap();
    let msg = format!("Deploy {} to gh-pages", sha);

    drop(fs::remove_dir_all(".git"));
    run(Command::new("git").arg("init"));
    run(Command::new("git").arg("config").arg("user.name").arg("Deploy from Travis CI"));
    run(Command::new("git").arg("config").arg("user.email").arg(""));
    run(Command::new("git").arg("add").arg("."));
    run(Command::new("git").arg("commit").arg("-m").arg(&msg));
    run(Command::new("git")
        .arg("push")
        .arg(format!("git@github.com:{}", slug))
        .arg("master:gh-pages")
        .env("SSH_AUTH_SOCK", &socket)
        .arg("-f"));
}

fn run(cmd: &mut Command) {
    println!("{:?}", cmd);
    let status = cmd.status().unwrap();
    assert!(status.success());
}

struct OnDrop<F: FnMut()>(F);

impl<F: FnMut()> Drop for OnDrop<F> {
    fn drop(&mut self) {
        (self.0)();
    }
}
