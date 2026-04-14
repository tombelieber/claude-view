use std::path::PathBuf;

pub fn install(log_dir: PathBuf, service_name: &'static str) {
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        prev_hook(info);

        let ts = chrono::Utc::now().format("%Y-%m-%dT%H-%M-%S");
        let pid = std::process::id();
        let crash_path = log_dir.join(format!("crash-{service_name}-{ts}-{pid:x}.log"));

        let msg = info
            .payload()
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| info.payload().downcast_ref::<String>().map(|s| s.as_str()))
            .unwrap_or("<non-string panic>");

        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "<unknown>".to_string());

        let thread = std::thread::current()
            .name()
            .map(|n| n.to_string())
            .unwrap_or_else(|| format!("{:?}", std::thread::current().id()));

        let backtrace = std::backtrace::Backtrace::force_capture();

        let report = format!(
            "PANIC\nmessage: {msg}\nlocation: {location}\nthread: {thread}\npid: {pid}\nservice: {service_name}\nbacktrace:\n{backtrace:?}\n"
        );

        let _ = std::fs::write(&crash_path, &report);
        tracing::error!(crash_file = %crash_path.display(), location = %location, "process.panic");
    }));
}
