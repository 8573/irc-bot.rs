use super::BotCmdAuthLvl;
use super::BotCmdHandler;
use super::BotCommand;
use super::Error;
use super::ErrorKind;
use super::GetDebugInfo;
use super::Result;
use super::State;
use itertools::Itertools;
use std;
use std::borrow::Cow;
use std::marker::PhantomData;
use uuid::Uuid;

pub struct Module<'modl> {
    pub name: Cow<'static, str>,
    uuid: Uuid,
    // TODO: once 1.18 is stable, make this pub_restricted to super.
    pub features: Vec<ModuleFeature<'modl>>,
    _lifetime: PhantomData<&'modl ()>,
}

impl<'modl> PartialEq for Module<'modl> {
    fn eq(&self, other: &Self) -> bool {
        if self.uuid == other.uuid {
            debug_assert_eq!(self.name, other.name);
            true
        } else {
            false
        }
    }
}

impl<'modl> Eq for Module<'modl> {}

impl<'modl> GetDebugInfo for Module<'modl> {
    type Output = ModuleInfo;

    fn dbg_info(&self) -> ModuleInfo {
        ModuleInfo { name: self.name.to_string() }
    }
}

pub struct ModuleBuilder<'modl> {
    name: Cow<'static, str>,
    features: Vec<ModuleFeature<'modl>>,
}

pub fn mk_module<'modl, S>(name: S) -> ModuleBuilder<'modl>
where
    S: Into<Cow<'static, str>>,
{
    ModuleBuilder {
        name: name.into(),
        features: Default::default(),
    }
}

impl<'modl> ModuleBuilder<'modl> {
    pub fn command<S1, S2, S3>(
        mut self,
        name: S1,
        syntax: S2,
        help_msg: S3,
        auth_lvl: BotCmdAuthLvl,
        handler: Box<BotCmdHandler>,
    ) -> Self
    where
        S1: Into<Cow<'static, str>>,
        S2: Into<Cow<'static, str>>,
        S3: Into<Cow<'static, str>>,
    {
        let name = name.into();

        assert!(
            !name.as_ref().contains(char::is_whitespace),
            "The name of the bot command {:?} contains whitespace, which is not allowed.",
            name.as_ref()
        );

        self.features.push(ModuleFeature::Command {
            name: name,
            usage: syntax.into(),
            help_msg: help_msg.into(),
            auth_lvl: auth_lvl,
            handler: handler,
            _lifetime: PhantomData,
        });

        self
    }

    pub fn end(self) -> Module<'modl> {
        let ModuleBuilder { name, mut features } = self;

        features.shrink_to_fit();

        Module {
            name: name,
            uuid: Uuid::new_v4(),
            features: features,
            _lifetime: PhantomData,
        }
    }
}

/// Information about a `Module` that can be gathered without needing any lifetime annotation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleInfo {
    name: String,
}

pub enum ModuleFeature<'modl> {
    Command {
        name: Cow<'static, str>,
        usage: Cow<'static, str>,
        help_msg: Cow<'static, str>,
        auth_lvl: BotCmdAuthLvl,
        handler: Box<BotCmdHandler>,
        _lifetime: PhantomData<&'modl ()>,
    },
    Trigger,
}

impl<'modl> GetDebugInfo for ModuleFeature<'modl> {
    type Output = ModuleFeatureInfo;

    fn dbg_info(&self) -> ModuleFeatureInfo {
        ModuleFeatureInfo {
            name: self.name().to_string(),
            kind: match self {
                &ModuleFeature::Command { .. } => ModuleFeatureKind::Command,
                &ModuleFeature::Trigger => ModuleFeatureKind::Trigger,
            },
        }
    }
}

/// Information about a `ModuleFeature` that can be gathered without needing any lifetime
/// annotation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleFeatureInfo {
    name: String,
    kind: ModuleFeatureKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModuleFeatureKind {
    Command,
    Trigger,
}

impl<'modl> ModuleFeature<'modl> {
    pub fn name(&self) -> &str {
        match self {
            &ModuleFeature::Command { ref name, .. } => name.as_ref(),
            &ModuleFeature::Trigger => unimplemented!(),
        }
    }

    // fn provider(&self) -> &Module {
    //     match self {
    //         &ModuleFeature::Command { provider, .. } => provider,
    //         &ModuleFeature::Trigger => unimplemented!(),
    //     }
    // }
}

impl<'modl> GetDebugInfo for BotCommand<'modl> {
    type Output = ModuleFeatureInfo;

    fn dbg_info(&self) -> ModuleFeatureInfo {
        ModuleFeatureInfo {
            name: self.name.to_string(),
            kind: ModuleFeatureKind::Command,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModuleLoadMode {
    /// Emit an error if any of the new module's features conflict with already present modules'
    /// features.
    Add,
    /// Overwrite any already loaded features that conflict with the new module's features, if the
    /// old features were provided by a module with the same name as the new module.
    Replace,
    /// Overwrite old modules' features unconditionally.
    Force,
}

impl<'server, 'modl> State<'server, 'modl> {
    pub fn load_modules<Modls>(
        &mut self,
        modules: Modls,
        mode: ModuleLoadMode,
    ) -> std::result::Result<(), Vec<Error>>
    where
        Modls: IntoIterator<Item = &'modl Module<'modl>>,
    {
        let errs = modules
            .into_iter()
            .filter_map(|module| match self.load_module(module, mode) {
                Ok(()) => None,
                Err(e) => Some(e),
            })
            .flatten()
            .collect::<Vec<Error>>();

        if errs.is_empty() { Ok(()) } else { Err(errs) }
    }

    pub fn load_module(
        &mut self,
        module: &'modl Module,
        mode: ModuleLoadMode,
    ) -> std::result::Result<(), Vec<Error>> {
        debug!(
            "Loading module {:?}, mode {:?}, providing {:?}",
            module.name,
            mode,
            module
                .features
                .iter()
                .map(GetDebugInfo::dbg_info)
                .collect::<Vec<_>>()
        );

        if let Some(existing_module) =
            match (mode, self.modules.get(module.name.as_ref())) {
                (_, None) |
                (ModuleLoadMode::Replace, _) |
                (ModuleLoadMode::Force, _) => None,
                (ModuleLoadMode::Add, Some(old)) => Some(old),
            }
        {
            return Err(vec![
                ErrorKind::ModuleRegistryClash(
                    existing_module.dbg_info(),
                    module.dbg_info()
                ).into(),
            ]);
        }

        self.modules.insert(module.name.clone(), module);

        let errs = module
            .features
            .iter()
            .filter_map(|feature| match self.load_module_feature(
                module,
                feature,
                mode,
            ) {
                Ok(()) => None,
                Err(e) => Some(e),
            })
            .collect::<Vec<Error>>();

        if errs.is_empty() { Ok(()) } else { Err(errs) }
    }

    fn load_module_feature(
        &mut self,
        provider: &'modl Module,
        feature: &'modl ModuleFeature,
        mode: ModuleLoadMode,
    ) -> Result<()> {
        debug!("Loading module feature (f1): {:?}", feature.dbg_info());

        if let Some(existing_feature) =
            match feature {
                &ModuleFeature::Command { .. } => {
                    match (mode, self.commands.get(feature.name())) {
                        (_, None) |
                        (ModuleLoadMode::Force, _) => None,
                        (ModuleLoadMode::Replace, Some(old))
                            if old.provider.name == provider.name => None,
                        (ModuleLoadMode::Replace, Some(old)) => Some(old.dbg_info()),
                        (ModuleLoadMode::Add, Some(old)) => Some(old.dbg_info()),
                    }
                }
                &ModuleFeature::Trigger => unimplemented!(),
            }
        {
            bail!(ErrorKind::ModuleFeatureRegistryClash(
                existing_feature,
                feature.dbg_info(),
            ))
        }

        self.force_load_module_feature(provider, feature);

        Ok(())
    }

    fn force_load_module_feature(
        &mut self,
        provider: &'modl Module,
        feature: &'modl ModuleFeature,
    ) {
        debug!("Loading module feature (f2): {:?}", feature.dbg_info());

        match feature {
            &ModuleFeature::Command {
                ref name,
                ref handler,
                ref auth_lvl,
                ref usage,
                ref help_msg,
                _lifetime: _,
            } => {
                self.commands.insert(
                    name.clone(),
                    BotCommand {
                        provider: provider,
                        name: name.clone(),
                        auth_lvl: auth_lvl.clone(),
                        handler: handler.as_ref(),
                        usage: usage.clone(),
                        help_msg: help_msg.clone(),
                    },
                )
            }
            &ModuleFeature::Trigger => unimplemented!(),
        };
    }
}
