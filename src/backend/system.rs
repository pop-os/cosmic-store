use appstream::{enums::ComponentKind, xmltree, Collection, Component};
use cosmic::widget;
use std::{
    collections::HashMap,
    error::Error,
    fs::{self, File},
    path::Path,
};

use super::{Backend, Package};
use crate::get_translatable;

pub struct System {
    installed: HashMap<String, (Package, Component)>,
}

impl System {
    fn load_metainfo(path: &Path, locale: &str) -> Result<(Package, Component), Box<dyn Error>> {
        let mut file = File::open(path)?;
        let element = xmltree::Element::parse(&mut file)?;
        let component = Component::try_from(&element)?;
        let id = component.id.to_string();
        if id.contains("FileRoller") {
            println!("{:#?}", component);
        }
        Ok((
            Package {
                id: id.clone(),
                //TODO: get icon from appstream data?
                icon: widget::icon::from_name(id.trim_end_matches(".desktop").to_string()),
                name: get_translatable(&component.name, &locale).to_string(),
                //TODO: get version
                version: "-".to_string(),
                extra: HashMap::new(),
            },
            component,
        ))
    }

    pub fn new() -> Result<Self, Box<dyn Error>> {
        let locale = sys_locale::get_locale().unwrap_or_else(|| {
            log::warn!("failed to get system locale, falling back to en-US");
            String::from("en-US")
        });

        //TODO: this is a massive hack to make getting appstream data easy
        let mut installed = HashMap::new();
        for entry_res in fs::read_dir("/usr/share/metainfo")? {
            let entry = entry_res?;
            let file_name_os = entry.file_name();
            let file_name = match file_name_os.to_str() {
                Some(some) => some,
                None => continue,
            };
            if !file_name.ends_with(".xml") {
                continue;
            }
            let path = entry.path();
            match Self::load_metainfo(&path, &locale) {
                Ok((package, component)) => {
                    //TODO: show non-desktop items?
                    if component.kind == ComponentKind::DesktopApplication {
                        installed.insert(package.id.clone(), (package, component));
                    }
                }
                Err(err) => {
                    log::warn!("failed to parse {:?}: {}", path, err);
                }
            }
        }

        Ok(Self { installed })
    }
}

impl Backend for System {
    fn installed(&self) -> Result<Vec<Package>, Box<dyn Error>> {
        Ok(self
            .installed
            .values()
            .map(|(package, _collection)| package.clone())
            .collect::<Vec<Package>>())
    }

    fn appstream(&self, package: &Package) -> Result<Collection, Box<dyn Error>> {
        match self.installed.get(&package.id) {
            Some((_package, component)) => {
                Ok(Collection {
                    //TODO: fill in more fields?
                    version: String::new(),
                    origin: None,
                    components: vec![component.clone()],
                    architecture: None,
                })
            }
            None => Err(format!("failed to find package {}", package.id).into()),
        }
    }
}
