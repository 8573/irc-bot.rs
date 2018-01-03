use super::ModuleFeatureInfo;
use super::ModuleInfo;
use irc;
use serde_yaml;
use std::borrow::Cow;
use std::io;
use std::sync::mpsc;
use util;

error_chain! {
    foreign_links {
        Io(io::Error);
        OutboxPush(mpsc::TrySendError<irc::proto::Message>);
        SerdeYaml(serde_yaml::Error);
    }

    links {
        IrcCrate(irc::error::Error, irc::error::ErrorKind);
        YamlUtil(util::yaml::Error, util::yaml::ErrorKind);
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
