//! Resolve only observed owners, once per UID for a finite command.

use std::collections::HashMap;
use sysinfo::Uid;

#[derive(Default)]
pub(super) struct UserNames {
    names: HashMap<Uid, Option<String>>,
}

impl UserNames {
    pub(super) fn new() -> Self {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            Self::default()
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            Self {
                names: sysinfo::Users::new_with_refreshed_list()
                    .iter()
                    .map(|user| (user.id().clone(), Some(user.name().into())))
                    .collect(),
            }
        }
    }

    pub(super) fn name(&mut self, uid: &Uid) -> Option<&str> {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            // getpwuid_r respects the host's account database (including NSS),
            // without enumerating every account or querying groups. Cache misses
            // too: an unknown owner must not trigger a lookup for every process.
            self.names
                .entry(uid.clone())
                .or_insert_with(|| {
                    nix::unistd::User::from_uid(nix::unistd::Uid::from_raw(**uid))
                        .ok()
                        .flatten()
                        .map(|user| user.name)
                })
                .as_deref()
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            self.names.get(uid).and_then(Option::as_deref)
        }
    }
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod tests {
    use super::*;

    #[test]
    fn resolves_the_real_owner_and_leaves_unknown_uids_unavailable() {
        let uid = nix::unistd::Uid::current()
            .as_raw()
            .to_string()
            .parse()
            .unwrap();
        let expected = std::process::Command::new("id")
            .args(["-u", "-n", "-r"])
            .output()
            .unwrap();
        assert!(expected.status.success());
        let expected = String::from_utf8(expected.stdout).unwrap();
        let expected = expected.trim_end_matches('\n');
        let mut names = UserNames::new();
        assert_eq!(names.name(&uid), Some(expected));
        assert_eq!(names.name(&uid), Some(expected));
        let unknown = u32::MAX.to_string().parse().unwrap();
        assert_eq!(names.name(&unknown), None);
    }
}
