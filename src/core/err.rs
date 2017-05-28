use super::ModuleFeatureInfo;
use super::ModuleInfo;
use std::borrow::Cow;
use std::io;

error_chain! {
    foreign_links {
        Io(io::Error);
    }

    errors {
        IdentificationFailure(io_err: io::Error)
        ModuleRegistryClash(old: ModuleInfo, new: ModuleInfo)
        ModuleFeatureRegistryClash(old: ModuleFeatureInfo, new: ModuleFeatureInfo)
        Config(key: String, problem: String)
        MsgPrefixUpdateRequestedButPrefixMissing
        ModuleRequestedQuit(quit_msg: Option<Cow<'static, str>>)
    }
}
