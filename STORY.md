# story

I like to write a short story of a repository's development sometimes.

## Sketching and Testing

Right now, I have a rough sketch of what my software is going to need, and what it's going to do.

- I might use this Tokio framework for idling and receiving notices while processing events. It seems like overkill,
but if I plan to use an automatic configuration reader, than it'd be useful.
- I am unsure at this time how to process and log events.
- Currently, creating a Windows service might be fast, but it also seems to make development slow.
    - Would supporting a separate stage Windows process be difficult? It seems like creating a service is just a bunch of boilerplate for running a process.
    - The process shows up in Task Manager like normal anyways.
- I'm unsure right now how to properly abstract the sleep/lock/idle states in both Windows and Linux. This might be really hard and annoying. Uggg.

Resources:
- [Minosse - Windows Service in Rust for setting Process Affinities automatically](https://github.com/artumino/minosse/tree/master)
- windows_service [Crates.io](https://crates.io/crates/windows-service/) [Docs](https://docs.rs/windows-service/latest/windows_service/)
- [windows_rs/samples](https://github.com/microsoft/windows-rs/tree/master/crates/samples)
- [Service Control Handler Function](https://learn.microsoft.com/en-us/windows/win32/services/service-control-handler-function?redirectedfrom=MSDN)
- [Event Logging using Rust in Windows](https://www.reddit.com/r/rust/comments/15cq9qp/event_logging_using_rust_in_windows/)
- daemonize [Crates.io](https://docs.rs/daemonize/latest/daemonize/)
- [SO - How to Programatically Detect When the OS Windows Is Waking Up or Going to Sleep](https://stackoverflow.com/questions/4693689/how-to-programmatically-detect-when-the-os-windows-is-waking-up-or-going-to-sl)
- [Tauri Discussion - Power Monitor api or plugin like in electron](https://github.com/tauri-apps/tauri/issues/8968)
- [Using WinAPI in Rust to Manage Windows Services](https://friendlyuser.github.io/posts/tech/rust/Using_WinAPI_in_Rust_to_Manage_Windows_Services/)

## Service vs Window

So initial studies were pretty confusing as most of the solutions for detecting changes in lock screen or sleep mode used `Wndproc`, which is a callback installed when creating a Window; a solution that I really wasn't interested in.

On top of being complex as hell, it required hiding the window manually using special `SW_HIDE` magic, and it sounded wrong. Also, I don't think services should have windows at all.