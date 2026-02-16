use flagfile_lib::{ff, Context};
use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex};

fn main() {
    let flag_name = "FF-something-else";
    let ctx: Context = HashMap::from([("tier", "premium".into()), ("country", "nl".into())]);

    // Shared signal so the main thread wakes up on each remote update.
    let notify = Arc::new((Mutex::new(false), Condvar::new()));
    let notify_clone = Arc::clone(&notify);

    flagfile_lib::init()
        .env("stage")
        .remote("http://127.0.0.1:8080")
        .token("rt_global_abc123")
        .on_update(move || {
            let (lock, cvar) = &*notify_clone;
            let mut updated = lock.lock().unwrap();
            *updated = true;
            cvar.notify_one();
        });

    // Initial evaluation
    println!("initial: {} = {:?}", flag_name, ff(flag_name, &ctx));

    // Block until Ctrl+C, re-evaluating the flag on every SSE update.
    let (lock, cvar) = &*notify;
    loop {
        let mut updated = lock.lock().unwrap();
        while !*updated {
            updated = cvar.wait(updated).unwrap();
        }
        *updated = false;

        println!("update received: {} = {:?}", flag_name, ff(flag_name, &ctx));
    }
}
