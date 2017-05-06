use super::BotCmdResult;
use super::MsgMetadata;
use super::State;

pub trait BotCmdHandler {
    fn run(&self, &State, &MsgMetadata, &str) -> BotCmdResult;
}

macro_rules! impl_fn {
    (($($param_id:ident: $param_ty:ty),*) => ($state_pat:pat, $msg_md_pat:pat, $arg_pat: pat)) => {
        impl<F, R> BotCmdHandler for F
            where F: Fn($($param_ty),*) -> R,
                  R: Into<BotCmdResult>
        {
            fn run(&self, $state_pat: &State, $msg_md_pat: &MsgMetadata, $arg_pat: &str)
                    -> BotCmdResult {
                self($($param_id),*).into()
            }
        }
    }
}

// I would like to allow functions taking any combination of the available arguments to be used as
// bot command handlers. However, it seems that rustc (versions 1.15.1, 1.17.0, 1.18.0, and 1.19.0)
// does not allow a trait to be implemented for multiple types of `Fn`.
//
// TODO: Occasionally check whether this has become allowed, using the test case that I have saved
// as <https://play.rust-lang.org/?gist=1d71b909f6e4adeddda89134031d4b1d>.

// impl_fn!((                                              ) => (_,     _,      _  ));
// impl_fn!((                                     arg: &str) => (_,     _,      arg));
// impl_fn!((               msg_md: &MsgMetadata           ) => (_,     msg_md, _  ));
// impl_fn!((               msg_md: &MsgMetadata, arg: &str) => (_,     msg_md, _  ));
// impl_fn!((state: &State                                 ) => (state, _,      _  ));
// impl_fn!((state: &State,                       arg: &str) => (state, _,      arg));
// impl_fn!((state: &State, msg_md: &MsgMetadata           ) => (state, msg_md, _  ));
impl_fn!(   (state: &State, msg_md: &MsgMetadata, arg: &str) => (state, msg_md, arg));
