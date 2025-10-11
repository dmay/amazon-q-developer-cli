You’ve got a few solid ways to make stdin reading “cancellable” in a Tokio app. Pick the trade-off that fits your platform and how fancy your input UX needs to be.

---

# 1) The simple, cross-platform pattern: `select!` with a CancellationToken + reader task

Put the line reader in its own task. When you cancel, abort that task. This avoids your main loop being held hostage by a pending `next_line()`.

```rust
use tokio::{io::{self, AsyncBufReadExt, BufReader}, task::JoinHandle};
use tokio_util::sync::CancellationToken;

async fn spawn_stdin_reader(
    tx: tokio::sync::mpsc::Sender<String>,
    cancel: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut lines = BufReader::new(io::stdin()).lines();
        loop {
            tokio::select! {
                _ = cancel.cancelled() => break, // cooperative exit
                line = lines.next_line() => {
                    match line {
                        Ok(Some(s)) => {
                            if tx.send(s).await.is_err() {
                                break; // consumer gone
                            }
                        }
                        Ok(None) => break, // EOF
                        Err(e) => {
                            eprintln!("stdin read error: {e}");
                            break;
                        }
                    }
                }
            }
        }
    })
}

#[tokio::main]
async fn main() {
    let cancel = CancellationToken::new();
    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(128);

    let reader = spawn_stdin_reader(tx, cancel.clone());

    // Example: some background task decides to stop the app
    let stopper = {
        let cancel = cancel.clone();
        tokio::spawn(async move {
            // ... do work ...
            cancel.cancel(); // request shutdown
        })
    };

    // Main app loop
    loop {
        tokio::select! {
            // consume user input if/when it arrives
            maybe_line = rx.recv() => {
                match maybe_line {
                    Some(line) => {
                        println!("got input: {line}");
                    }
                    None => break, // reader ended or app is shutting down
                }
            }
            // observe global cancellation
            _ = cancel.cancelled() => {
                break;
            }
        }
    }

    // Ensure reader task is gone. If it's still stuck, abort it.
    // (This is defensive mainly for Windows, where console reads can block a helper thread.)
    if !reader.is_finished() {
        reader.abort();
        let _ = reader.await; // swallow JoinError
    }
    let _ = stopper.await;
}
```

**Why this works well**

* Your main loop is never awaiting `next_line()` directly, so it can break on cancel instantly.
* On Unix and most setups, the reader task will stop right away via the `cancelled()` branch. On platforms where stdin is backed by a blocking console read (notably Windows), `abort()` ensures the task doesn’t keep your process alive.

---

# 2) `select!` directly around `next_line()` (Unix-friendly)

If you do want to read inline (no separate task), wrap the await:

```rust
tokio::select! {
    _ = cancel.cancelled() => { break; }
    res = lines.next_line() => {
        match res {
            Ok(Some(line)) => { /* handle */ }
            Ok(None) => break, // EOF
            Err(e) => { eprintln!("read error: {e}"); break; }
        }
    }
}
```

This works well on Unix TTYs/pipes: dropping the future cancels the pending poll. On Windows consoles, there are known limitations (Tokio uses a blocking thread under the hood), so your process may not exit until a line arrives. If you need robust Windows behavior, prefer #1 or #4.

---

# 3) Add a timeout around reads (polling style)

Poll stdin periodically so you can check cancellation between polls:

```rust
use std::time::Duration;

loop {
    tokio::select! {
        _ = cancel.cancelled() => break,
        res = tokio::time::timeout(Duration::from_millis(200), lines.next_line()) => {
            match res {
                Ok(Ok(Some(line))) => { /* handle */ }
                Ok(Ok(None)) => break,             // EOF
                Ok(Err(e)) => { eprintln!("{e}"); break; }
                Err(_elapsed) => { /* timed out: loop to recheck cancel */ }
            }
        }
    }
}
```

**Trade-off:** tiny latency and extra wakeups; simplest to retrofit.

---

# 4) Raw/“event” mode input (no line buffering) via a TUI crate

If you’re already considering richer TUI, put the terminal in raw mode and read key events rather than `lines()`. Libraries like **crossterm** or **rustyline** help here. With crossterm you can poll with a timeout and exit immediately on cancel, no waiting for Enter:

```rust
use crossterm::{event, terminal, ExecutableCommand};
use std::io::stdout;
use std::time::Duration;

fn enter_raw_mode() -> crossterm::Result<()> {
    stdout().execute(terminal::EnterAlternateScreen)?;
    terminal::enable_raw_mode()
}

fn leave_raw_mode() -> crossterm::Result<()> {
    terminal::disable_raw_mode()?;
    stdout().execute(terminal::LeaveAlternateScreen)?;
    Ok(())
}

async fn input_loop(cancel: CancellationToken) -> crossterm::Result<()> {
    let _guard = scopeguard::guard((), |_| { let _ = leave_raw_mode(); });
    enter_raw_mode()?;

    loop {
        // Non-blocking poll: returns after timeout even if no key was pressed
        if event::poll(Duration::from_millis(100))? {
            if let event::Event::Key(k) = event::read()? {
                // build your own line buffer if you want line semantics
                // or handle keys directly
                if k.code == event::KeyCode::Esc { break; }
            }
        }
        if cancel.is_cancelled() { break; }
    }
    Ok(())
}
```

**Pros:** instant cancellation, richer UI.
**Cons:** you implement line editing yourself (or use a library that supports it).

---

# 5) Unix-only: make stdin non-canonical/non-blocking

If you control the runtime environment (Linux/macOS), you can:

* put the tty into non-canonical mode with `termios` (no newline required),
* or set the stdin fd to `O_NONBLOCK` and read bytes yourself,
* wrap the fd in `tokio::io::unix::AsyncFd` to integrate with the reactor.

This gives you fine-grained control and instant cancellation, but it’s lower-level and not portable to Windows consoles.

---

## Which should you use?

* **Need portability and fast exits?** Do **#1 (reader task + abort)**. It’s the most practical and least invasive.
* **Only targeting Unix and want minimal code?** **#2** is fine.
* **Building a real TUI anyway?** Move to **#4** and forget about `lines()`. It’s the best UX.

If you want, paste a snippet of your current main loop/cancel path and I’ll adapt it to the reader-task pattern with your types (e.g., `tokio_util::sync::CancellationToken`, `watch`/`Notify`, etc.).
