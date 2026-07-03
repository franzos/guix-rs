use std::fs;

use libguix::{BaselineSource, SystemBaseline};

fn write(dir: &std::path::Path, name: &str, body: &str) -> std::path::PathBuf {
    let p = dir.join(name);
    fs::write(&p, body).expect("write fixture");
    p
}

const ETC: &str = "(list (channel (name 'pantherx) (url \"https://codeberg.org/gofranz/panther.git\") (branch \"master\") (introduction (make-channel-introduction \"54b4056ac571611892c743b65f4c47dc298c49da\" (openpgp-fingerprint \"A36A D41E ECC7 A871 1003 5D24 524F EB1A 9D33 C9CB\")))) (channel (name 'guix) (url \"https://git.guix.gnu.org/guix.git\") (branch \"master\")))";

const PROVENANCE: &str = "(list (channel (name 'guix) (url \"https://git.guix.gnu.org/guix.git\") (branch \"master\") (commit \"2983d36030ba74037667b28012c76d4bf35e459f\")) (channel (name 'nonguix) (url \"https://gitlab.com/nonguix/nonguix.git\") (branch \"master\") (commit \"d35a2f8f22023426ccf3598fa7079b09bb821e3e\")))";

#[tokio::test]
async fn etc_wins_and_is_verbatim() {
    let dir = tempfile::tempdir().unwrap();
    let etc = write(dir.path(), "etc.scm", ETC);
    let prov = write(dir.path(), "prov.scm", PROVENANCE);
    let b = SystemBaseline::read(Some(&etc), Some(&prov)).await;
    assert_eq!(b.source, BaselineSource::Etc);
    assert!(b.etc_present());
    let names: Vec<&str> = b.channels.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, vec!["pantherx", "guix"]);
    // /etc is branch-only already, no commits.
    assert!(b.channels.iter().all(|c| c.commit.is_none()));
    assert!(b.contains("pantherx"));
}

#[tokio::test]
async fn provenance_used_when_etc_absent_and_commits_stripped() {
    let dir = tempfile::tempdir().unwrap();
    let etc = dir.path().join("does-not-exist.scm");
    let prov = write(dir.path(), "prov.scm", PROVENANCE);
    let b = SystemBaseline::read(Some(&etc), Some(&prov)).await;
    assert_eq!(b.source, BaselineSource::Provenance);
    assert!(!b.etc_present());
    let names: Vec<&str> = b.channels.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, vec!["guix", "nonguix"]);
    // Provenance commits must be stripped so channels track their branch.
    assert!(b.channels.iter().all(|c| c.commit.is_none()));
}

#[tokio::test]
async fn defaults_when_neither_present() {
    let dir = tempfile::tempdir().unwrap();
    let etc = dir.path().join("nope-a.scm");
    let prov = dir.path().join("nope-b.scm");
    let b = SystemBaseline::read(Some(&etc), Some(&prov)).await;
    assert_eq!(b.source, BaselineSource::Defaults);
    assert!(b.channels.is_empty());
}
