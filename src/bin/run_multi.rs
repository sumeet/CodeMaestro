// there's no sh on this system, and we want to have the docker entrypoint invoke multiple
// commands. lol

use std::process::{Command, Stdio};

fn main() {
    // skip 1 to get to the first arg, otherwise it's the binary
    for command in std::env::args().skip(1) {
        let command_and_args = command.split(" ").collect::<Vec<_>>();
        if command_and_args.is_empty() {
            continue;
        }

        let (cmd, args) = command_and_args.split_first().unwrap();
        let mut cmd = Command::new(cmd);
        cmd.stderr(Stdio::inherit())
           .stdout(Stdio::inherit())
           .args(args);
        println!("running {:?}", cmd);
        let output = cmd.output().unwrap();
        if !output.status.success() {
            println!("command failed, exiting");
            break;
        }
        println!();
    }
}
