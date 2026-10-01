//! `chown`/`chgrp`/`lchown`/`lchgrp` on the system libc, like
//! ext/standard/filestat.c: a name is resolved via getpwnam/getgrnam, a
//! numeric id is passed through; -1 = "don't change" (POSIX chown semantics).

use std::ffi::CString;

/// uid for a user name, `None` if unknown (or the name contains NUL).
pub fn resolve_uid(name: &[u8]) -> Option<u32> {
    let c = CString::new(name).ok()?;
    // SAFETY: getpwnam on a valid CString; the pointer is read immediately.
    let pw = unsafe { libc::getpwnam(c.as_ptr()) };
    if pw.is_null() { None } else { Some(unsafe { (*pw).pw_uid }) }
}

/// gid for a group name, `None` if unknown.
pub fn resolve_gid(name: &[u8]) -> Option<u32> {
    let c = CString::new(name).ok()?;
    // SAFETY: getgrnam on a valid CString; the pointer is read immediately.
    let gr = unsafe { libc::getgrnam(c.as_ptr()) };
    if gr.is_null() { None } else { Some(unsafe { (*gr).gr_gid }) }
}

/// chown(2)/lchown(2): a `uid`/`gid` of -1 leaves that side unchanged.
pub fn change_owner(path: &[u8], uid: i64, gid: i64, follow: bool) -> Result<(), std::io::Error> {
    let c = CString::new(path)
        .map_err(|_| std::io::Error::from_raw_os_error(libc::ENOENT))?;
    // SAFETY: valid path CString; -1 as uid_t/gid_t = "don't change" POSIX.
    let rc = unsafe {
        if follow {
            libc::chown(c.as_ptr(), uid as libc::uid_t, gid as libc::gid_t)
        } else {
            libc::lchown(c.as_ptr(), uid as libc::uid_t, gid as libc::gid_t)
        }
    };
    if rc == 0 { Ok(()) } else { Err(std::io::Error::last_os_error()) }
}
