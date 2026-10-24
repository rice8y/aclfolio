//! Offline integration tests for opt-in read-oriented I/O.
#[cfg(feature = "network")]
mod downloads {
    use aclfolio::{
        config::Config,
        files::{DownloadError, DownloadOutcome, FileKind, FileReference, compute_checksum},
    };
    use std::{
        io::{Read, Write},
        net::TcpListener,
        time::Duration,
    };
    fn server(
        status: &'static str,
        content_type: Option<&'static str>,
        body: &'static [u8],
    ) -> (String, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/test", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0; 4096];
            let _ = stream.read(&mut buf).unwrap();
            write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n",
                body.len()
            )
            .unwrap();
            if let Some(t) = content_type {
                write!(stream, "Content-Type: {t}\r\n").unwrap();
            }
            stream.write_all(b"\r\n").unwrap();
            stream.write_all(body).unwrap();
        });
        (url, handle)
    }
    #[test]
    fn checksum_skip_download_warning_and_content_type() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("paper.pdf");
        let data = b"%PDF test";
        let (url, server) = server("200 OK", Some("application/pdf"), data);
        let reference = FileReference::new(FileKind::Pdf, url, Some(compute_checksum(data)));
        assert_eq!(
            reference
                .download(&path, &Config::default(), Duration::from_secs(2))
                .unwrap(),
            DownloadOutcome::Downloaded
        );
        server.join().unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), data);
        // The server has exited: this verifies the checksum shortcut avoids any HTTP request.
        assert_eq!(
            reference
                .download(&path, &Config::default(), Duration::from_secs(2))
                .unwrap(),
            DownloadOutcome::AlreadyPresent
        );
        let (url, server) = self::server("200 OK", Some("application/pdf"), b"changed");
        let reference = FileReference::new(FileKind::Pdf, url, Some("00000000".into()));
        assert_eq!(
            reference
                .download(&path, &Config::default(), Duration::from_secs(2))
                .unwrap(),
            DownloadOutcome::ChecksumMismatch
        );
        server.join().unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"changed");
        let (url, server) = self::server("200 OK", Some("text/html"), b"unexpected");
        let reference = FileReference::new(FileKind::Pdf, url, None);
        assert!(matches!(
            reference.download(&path, &Config::default(), Duration::from_secs(2)),
            Err(DownloadError::ContentType { .. })
        ));
        server.join().unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"changed");
        let (url, server) = self::server("404 Not Found", None, b"missing");
        let reference = FileReference::new(FileKind::Url, url, None);
        assert!(matches!(
            reference.download(&path, &Config::default(), Duration::from_secs(2)),
            Err(DownloadError::Http(_))
        ));
        server.join().unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"changed");
    }
}
#[cfg(feature = "git")]
mod repositories {
    use aclfolio::repository::{clone_or_pull_from_repo, data_directory_within_repo};
    use std::{path::Path, process::Command};
    fn git(path: &Path, args: &[&str]) {
        let out = Command::new("git")
            .arg("-C")
            .arg(path)
            .args([
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    #[test]
    fn shallow_clone_pull_and_origin_warning() {
        let dir = tempfile::tempdir().unwrap();
        let origin = dir.path().join("origin");
        let checkout = dir.path().join("checkout");
        std::fs::create_dir(&origin).unwrap();
        git(&origin, &["init", "--initial-branch=main"]);
        std::fs::create_dir(origin.join("data")).unwrap();
        std::fs::write(origin.join("data/version"), "one").unwrap();
        git(&origin, &["add", "."]);
        git(&origin, &["commit", "-m", "one"]);
        let url = format!("file://{}", origin.display());
        assert_eq!(clone_or_pull_from_repo(&url, &checkout).unwrap(), None);
        assert_eq!(
            std::fs::read_to_string(checkout.join("data/version")).unwrap(),
            "one"
        );
        std::fs::write(origin.join("data/version"), "two").unwrap();
        git(&origin, &["add", "."]);
        git(&origin, &["commit", "-m", "two"]);
        assert_eq!(clone_or_pull_from_repo(&url, &checkout).unwrap(), None);
        assert_eq!(
            std::fs::read_to_string(checkout.join("data/version")).unwrap(),
            "two"
        );
        assert!(
            clone_or_pull_from_repo("https://example.invalid/other", &checkout)
                .unwrap()
                .is_some()
        );
        assert_eq!(
            std::fs::canonicalize(data_directory_within_repo(checkout.join("data")).unwrap())
                .unwrap(),
            std::fs::canonicalize(checkout.join("data")).unwrap()
        );
    }
}
