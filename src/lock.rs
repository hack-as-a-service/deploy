use std::fs;

use directories::UserDirs;

lazy_static! {
    static ref DIRS: UserDirs = UserDirs::new().unwrap();
}

pub fn is_locked(name: &str) -> bool {
    let path = DIRS.home_dir().join(format!(".{}_deploy_lock", name));

    path.exists()
}

pub fn lock(name: &str) {
    let path = DIRS.home_dir().join(format!(".{}_deploy_lock", name));

    fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(path)
        .expect("Error setting deploy lock");
}

pub fn unlock(name: &str) {
    let path = DIRS.home_dir().join(format!(".{}_deploy_lock", name));

    fs::remove_file(path).expect("Error releasing deploy lock");
}
