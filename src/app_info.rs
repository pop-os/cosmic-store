use appstream::{enums::Icon, Component};

fn get_translatable<'a>(translatable: &'a appstream::TranslatableString, locale: &str) -> &'a str {
    match translatable.get_for_locale(locale) {
        Some(some) => some.as_str(),
        None => match translatable.get_default() {
            Some(some) => some.as_str(),
            None => "",
        },
    }
}

/*TODO: handle p tags with xml:lang
fn get_markup_translatable<'a>(
    translatable: &'a appstream::MarkupTranslatableString,
    locale: &str,
) -> &'a str {
    match translatable.get_for_locale(locale) {
        Some(some) => some.as_str(),
        None => match translatable.get_default() {
            Some(some) => some.as_str(),
            None => "",
        },
    }
}
*/

#[derive(Debug)]
pub struct AppInfo {
    pub origin_opt: Option<String>,
    pub name: String,
    pub summary: String,
    pub pkgname: Option<String>,
    pub icons: Vec<Icon>,
}

impl AppInfo {
    pub fn new(origin_opt: Option<String>, component: Component, locale: &str) -> Self {
        let name = get_translatable(&component.name, locale);
        let summary = component
            .summary
            .as_ref()
            .map_or("", |x| get_translatable(x, locale));
        /*TODO: MarkupTranslatableString doesn't properly filter p tag with xml:lang
        if let Some(description) = &component.description {
            column = column.push(widget::text(get_markup_translatable(
                description,
                &self.locale,
            )));
        }
        */
        Self {
            origin_opt,
            name: name.to_string(),
            summary: summary.to_string(),
            pkgname: component.pkgname,
            icons: component.icons,
        }
    }
}
