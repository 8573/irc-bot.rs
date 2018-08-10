use super::ModuleFeatureInfo;
use super::ModuleInfo;
use super::ServerId;
use irc;
use rand;
use serde_yaml;
use std::any::Any;
use std::borrow::Cow;
use std::io;
use util;
use walkdir;

error_chain! {
    foreign_links {
        Io(io::Error);

        Rand(rand::Error);

        SerdeYaml(serde_yaml::Error);

        WalkDir(walkdir::Error);
    }

    links {
        YamlUtil(util::yaml::Error, util::yaml::ErrorKind);
    }

    errors {
        // TODO: Once I switch from `error-chain` to `failure`, integrate with `irc`'s `failure`
        // support.
        IrcCrate(inner: irc::error::IrcError) {
            description("IRC error")
            display("IRC error: {}", inner)
        }

        ModuleRegistryClash(old: ModuleInfo, new: ModuleInfo) {
            description("module registry clash")
            display("Failed to load a new module because it would have overwritten an old module. \
                     Old: {:?}; new: {:?}.",
                    old,
                    new)
        }

        ModuleFeatureRegistryClash(old: ModuleFeatureInfo, new: ModuleFeatureInfo) {
            description("module feature registry clash")
            display("Failed to load a new module feature because it would have overwritten an old \
                     module feature. Old: {:?}; new: {:?}.",
                    old,
                    new)
        }

        ServerRegistryClash(server_id: ServerId) {
            description("server registry UUID clash")
            display("Failed to register a server because an existing server had the same UUID: \
                     {uuid}",
                    uuid = server_id.uuid.hyphenated())
        }

        Config(key: String, problem: String) {
            description("configuration error")
            display("Configuration error: Key {:?} {}.", key, problem)
        }

        ThreadSpawnFailure(io_err: io::Error) {
            description("failed to spawn thread")
            display("Failed to spawn thread: {}", io_err)
        }

        HandlerPanic(
            feature_kind: Cow<'static, str>,
            feature_name: Cow<'static, str>,
            payload: Box<Any + Send + 'static>
        ) {
            description("panic in module feature handler function")
            display("The handler function for {} {:?} panicked with the following message: {}",
                    feature_kind,
                    feature_name,
                    util::fmt::FmtAny(payload.as_ref()))
        }

        NicknameUnknown {
            description("nickname retrieval error")
            display("Puzzlingly, the bot seems to have forgotten its own nickname.")
        }

        ReceivedMsgHasBadPrefix {
            description("an operation failed because a received message had a malformed prefix \
                         (the part that identifies the sender)")
            display("An operation failed because a message was received that had a malformed \
                     prefix (the part that identifies the sender).")
        }

        UnknownServer(server_id: ServerId) {
            description("server ID not recognized")
            display("An attempt to look up a server connection or metadatum thereof failed, \
                     because the given server identification token (UUID {id}) was not a valid \
                     key in the relevant associative array.",
                    id = server_id.uuid.hyphenated())
        }

        LockPoisoned(lock_contents_desc: Cow<'static, str>) {
            description("lock poisoned")
            display("A thread panicked, poisoning a lock around {}.", lock_contents_desc)
        }

        Any(inner: Box<Any + Send + 'static>) {
            description("miscellaneous error")
            display("Error: {}", util::fmt::FmtAny(inner.as_ref()))
        }

        Unit {
            description("unknown error")
            display("An error seems to have occurred, but unfortunately the error type provided \
                     was the unit type, containing no information about the error.")
        }
    }
}

impl From<irc::error::IrcError> for Error {
    fn from(orig: irc::error::IrcError) -> Self {
        ErrorKind::IrcCrate(orig).into()
    }
}
