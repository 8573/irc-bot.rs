use super::ModuleFeatureInfo;
use super::ModuleInfo;
use super::config;
use pircolate;
use std;
use std::borrow::Cow;
use std::io;
use std::sync;
use std::sync::mpsc;
use tokio_irc_client;

error_chain! {
    foreign_links {
        Io(io::Error);
        OutboxPush(mpsc::TrySendError<pircolate::Message>);
    }

    links {
        Pircolate(pircolate::error::Error, pircolate::error::ErrorKind);
        TokioIrcClient(tokio_irc_client::error::Error, tokio_irc_client::error::ErrorKind);
    }

    errors {
        IdentificationFailure(io_err: io::Error)
        ModuleRegistryClash(old: ModuleInfo, new: ModuleInfo)
        ModuleFeatureRegistryClash(old: ModuleFeatureInfo, new: ModuleFeatureInfo)
        Config(key: String, problem: String) {
            description("configuration error")
            display("Configuration error: Key {:?} {}.", key, problem)
        }
        MsgPrefixUpdateRequestedButPrefixMissing
        ModuleRequestedQuit(quit_msg: Option<Cow<'static, str>>)
        NicknameUnknown {
            description("nickname retrieval error")
            display("Puzzlingly, the bot seems to have forgotten its own nickname.")
        }
        Unit {
            description("unknown error")
            display("An error seems to have occurred, but unfortunately the error type provided \
                     was the unit type, containing no information about the error.")
        }
    }
}
