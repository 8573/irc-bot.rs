use super::trigger::TriggerPriority;
use super::BotCmdAttr;
use super::BotCmdAuthLvl;
use super::BotCmdHandler;
use super::BotCommand;
use super::Error;
use super::ErrorKind;
use super::GetDebugInfo;
use super::ModuleLoadHandler;
use super::Result;
use super::State;
use super::Trigger;
use super::TriggerAttr;
use super::TriggerHandler;
use itertools;
use regex::Regex;
use smallvec::SmallVec;
use std;
use std::borrow::Cow;
use std::sync::Arc;
use std::sync::RwLock;
use util;
use uuid::Uuid;
use yaml_rust::Yaml;

#[derive(CustomDebug)]
pub struct Module {
    pub name: Cow<'static, str>,

    #[debug(with = "util::fmt::debug_uuid")]
    uuid: Uuid,

    features: Vec<ModuleFeature>,

    #[debug(skip)]
    on_load: SmallVec<[Box<ModuleLoadHandler>; 1]>,
}

impl PartialEq for Module {
    fn eq(&self, other: &Self) -> bool {
        if self.uuid == other.uuid {
            debug_assert_eq!(self.name, other.name);
            true
        } else {
            false
        }
    }
}

impl Eq for Module {}

impl GetDebugInfo for Module {
    type Output = ModuleInfo;

    fn dbg_info(&self) -> ModuleInfo {
        ModuleInfo {
            name: self.name.to_string(),
        }
    }
}

pub struct ModuleBuilder {
    name: Cow<'static, str>,
    features: Vec<ModuleFeature>,
    on_load: SmallVec<[Box<ModuleLoadHandler>; 1]>,
}

pub fn mk_module<'modl, S>(name: S) -> ModuleBuilder
where
    S: Into<Cow<'static, str>>,
{
    ModuleBuilder {
        name: name.into(),
        features: Default::default(),
        on_load: Default::default(),
    }
}

impl ModuleBuilder {
    pub fn command<'attr, Attrs, S1, S2, S3>(
        mut self,
        name: S1,
        syntax: S2,
        help_msg: S3,
        auth_lvl: BotCmdAuthLvl,
        handler: Box<BotCmdHandler>,
        attrs: Attrs,
    ) -> Self
    where
        S1: Into<Cow<'static, str>>,
        S2: Into<Cow<'static, str>>,
        S3: Into<Cow<'static, str>>,
        Attrs: IntoIterator<Item = &'attr BotCmdAttr>,
    {
        let name = name.into();

        assert!(
            !name.as_ref().contains(char::is_whitespace),
            "The name of the bot command {:?} contains whitespace, which is not allowed.",
            name.as_ref()
        );

        let syntax = syntax.into();
        let usage_yaml = util::yaml::parse_node(&syntax)
            .unwrap()
            .unwrap_or(Yaml::Hash(Default::default()));

        let cmd = ModuleFeature::Command {
            name: name,
            usage_str: syntax,
            usage_yaml,
            help_msg: help_msg.into(),
            auth_lvl: auth_lvl,
            handler: handler.into(),
        };

        for attr in attrs {
            match *attr {
                // ...
            }
        }

        self.features.push(cmd);

        self
    }

    pub fn trigger<'attr, Attrs, Rx1, S1, S2>(
        mut self,
        name: S1,
        regex_str: Rx1,
        help_msg: S2,
        priority: TriggerPriority,
        handler: Box<TriggerHandler>,
        attrs: Attrs,
    ) -> Self
    where
        Rx1: util::regex::IntoRegexCI,
        S1: Into<Cow<'static, str>>,
        S2: Into<Cow<'static, str>>,
        Attrs: IntoIterator<Item = &'attr TriggerAttr>,
    {
        for attr in attrs {
            match attr {
                &TriggerAttr::AlwaysWatching => unimplemented!(),
            }
        }

        let trigger = ModuleFeature::Trigger {
            name: name.into(),
            regex: Arc::new(RwLock::new(regex_str.into_regex_ci().expect(
                "Your regex was erroneous, it \
                 seems.",
            ))),
            help_msg: help_msg.into(),
            handler: handler.into(),
            priority,
            uuid: Uuid::new_v4(),
        };

        self.features.push(trigger);

        self
    }

    /// Sets a handler function for loading or reloading the module's configuration.
    ///
    /// The given `handler` function will be called when the module is first loaded, as well as if
    /// module configurations are reloaded, which may happen at a bot administrator's request.
    ///
    /// Multiple such handler functions may be set, by calling this function multiple times, but it
    /// generally likely would be better to set a single handler function that calls multiple
    /// sub-handlers.
    pub fn on_load(mut self, handler: Box<ModuleLoadHandler>) -> Self {
        self.on_load.push(handler);

        self
    }

    pub fn end(self) -> Module {
        let ModuleBuilder {
            name,
            mut features,
            mut on_load,
        } = self;

        features.shrink_to_fit();
        on_load.shrink_to_fit();

        Module {
            name: name,
            uuid: Uuid::new_v4(),
            features: features,
            on_load,
        }
    }
}

/// Information about a `Module` that can be gathered without needing any lifetime annotation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleInfo {
    name: String,
}

#[derive(CustomDebug)]
enum ModuleFeature {
    Command {
        name: Cow<'static, str>,

        usage_str: Cow<'static, str>,

        usage_yaml: Yaml,

        help_msg: Cow<'static, str>,

        auth_lvl: BotCmdAuthLvl,

        #[debug(skip)]
        handler: Arc<BotCmdHandler>,
    },
    Trigger {
        name: Cow<'static, str>,

        help_msg: Cow<'static, str>,

        regex: Arc<RwLock<Regex>>,

        #[debug(skip)]
        handler: Arc<TriggerHandler>,

        priority: TriggerPriority,

        uuid: Uuid,
    },
}

impl GetDebugInfo for ModuleFeature {
    type Output = ModuleFeatureInfo;

    fn dbg_info(&self) -> ModuleFeatureInfo {
        ModuleFeatureInfo {
            name: self.name().to_string(),
            kind: match self {
                &ModuleFeature::Command { .. } => ModuleFeatureKind::Command,
                &ModuleFeature::Trigger { .. } => ModuleFeatureKind::Trigger,
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

impl ModuleFeature {
    pub fn name(&self) -> &str {
        match self {
            &ModuleFeature::Command { ref name, .. } => name.as_ref(),
            &ModuleFeature::Trigger { ref name, .. } => name.as_ref(),
        }
    }

    // fn provider(&self) -> &Module {
    //     match self {
    //         &ModuleFeature::Command { provider, .. } => provider,
    //         &ModuleFeature::Trigger => unimplemented!(),
    //     }
    // }
}

impl GetDebugInfo for BotCommand {
    type Output = ModuleFeatureInfo;

    fn dbg_info(&self) -> ModuleFeatureInfo {
        ModuleFeatureInfo {
            name: self.name.to_string(),
            kind: ModuleFeatureKind::Command,
        }
    }
}

impl GetDebugInfo for Trigger {
    type Output = ModuleFeatureInfo;

    fn dbg_info(&self) -> ModuleFeatureInfo {
        ModuleFeatureInfo {
            name: self.name.to_string(),
            kind: ModuleFeatureKind::Trigger,
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

impl State {
    pub fn load_modules<Modls>(
        &mut self,
        modules: Modls,
        mode: ModuleLoadMode,
    ) -> std::result::Result<(), Vec<Error>>
    where
        Modls: IntoIterator<Item = Module>,
    {
        let errs = itertools::flatten(modules.into_iter().filter_map(|module| {
            match self.load_module(module, mode) {
                Ok(()) => None,
                Err(e) => Some(e),
            }
        }))
        .collect::<Vec<Error>>();

        if errs.is_empty() {
            Ok(())
        } else {
            Err(errs)
        }
    }

    pub fn load_module(
        &mut self,
        module: Module,
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

        if let Some(existing_module) = match (mode, self.modules.get(module.name.as_ref())) {
            (_, None) | (ModuleLoadMode::Replace, _) | (ModuleLoadMode::Force, _) => None,
            (ModuleLoadMode::Add, Some(old)) => Some(old),
        } {
            return Err(vec![ErrorKind::ModuleRegistryClash(
                existing_module.dbg_info(),
                module.dbg_info(),
            )
            .into()]);
        }

        let module = Arc::new(module);

        self.modules.insert(module.name.clone(), module.clone());

        let errs = module
            .features
            .iter()
            .filter_map(
                |feature| match self.load_module_feature(module.clone(), feature, mode) {
                    Ok(()) => None,
                    Err(e) => Some(e),
                },
            )
            .collect::<Vec<Error>>();

        if !errs.is_empty() {
            return Err(errs);
        }

        for handler in &module.on_load {
            match handler.run(self) {
                Ok(()) => {}
                Err(err) => return Err(vec![err]),
            }
        }

        Ok(())
    }

    fn load_module_feature<'modl>(
        &mut self,
        provider: Arc<Module>,
        feature: &'modl ModuleFeature,
        mode: ModuleLoadMode,
    ) -> Result<()> {
        trace!("Loading module feature (phase 1): {:?}", feature.dbg_info());

        if let Some(existing_feature) = match feature {
            &ModuleFeature::Command { .. } => match (mode, self.commands.get(feature.name())) {
                (_, None) | (ModuleLoadMode::Force, _) => None,
                (ModuleLoadMode::Replace, Some(old)) if old.provider.name == provider.name => None,
                (ModuleLoadMode::Replace, Some(old)) => Some(old.dbg_info()),
                (ModuleLoadMode::Add, Some(old)) => Some(old.dbg_info()),
            },
            &ModuleFeature::Trigger { .. } => None,
        } {
            bail!(ErrorKind::ModuleFeatureRegistryClash(
                existing_feature,
                feature.dbg_info(),
            ))
        }

        self.force_load_module_feature(provider, feature);

        Ok(())
    }

    fn force_load_module_feature<'modl>(
        &mut self,
        provider: Arc<Module>,
        feature: &'modl ModuleFeature,
    ) {
        trace!("Loading module feature (phase 2): {:?}", feature.dbg_info());

        match feature {
            &ModuleFeature::Command {
                ref name,
                ref handler,
                ref auth_lvl,
                ref usage_str,
                ref usage_yaml,
                ref help_msg,
            } => {
                self.commands.insert(
                    name.clone(),
                    BotCommand {
                        provider: provider,
                        name: name.clone(),
                        auth_lvl: auth_lvl.clone(),
                        handler: handler.clone(),
                        usage_str: usage_str.clone(),
                        usage_yaml: usage_yaml.clone(),
                        help_msg: help_msg.clone(),
                    },
                );
            }
            &ModuleFeature::Trigger {
                ref name,
                ref regex,
                ref handler,
                ref help_msg,
                priority,
                uuid,
            } => {
                self.triggers
                    .entry(priority)
                    .or_insert_with(Default::default)
                    .push(Trigger {
                        provider,
                        name: name.clone(),
                        regex: regex.clone(),
                        handler: handler.clone(),
                        priority,
                        help_msg: help_msg.clone(),
                        uuid,
                    });
            }
        };
    }
}
