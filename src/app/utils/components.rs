#![allow(unused)]

use std::borrow::Cow;

use serenity::{
    builder::{
        CreateComponent, CreateContainer, CreateContainerComponent, CreateSection, CreateSectionAccessory,
        CreateSectionComponent, CreateSeparator, CreateTextDisplay, CreateThumbnail, CreateUnfurledMediaItem,
    },
    model::{Color, application::SeparatorSpacingSize},
};

pub fn create_thumbnail<'a>(url: impl Into<Cow<'a, str>>) -> CreateThumbnail<'a> {
    CreateThumbnail::new(CreateUnfurledMediaItem::new(url))
}

pub fn create_container<'a>(
    components: impl Into<Cow<'a, [CreateContainerComponent<'a>]>>,
    color: Option<impl Into<Color>>,
    spoiler: bool,
) -> CreateComponent<'a> {
    let mut container = CreateContainer::new(components);

    if let Some(color) = color {
        container = container.accent_color(color.into());
    }

    if spoiler {
        container = container.spoiler(spoiler);
    }

    CreateComponent::Container(container)
}

fn _create_separator<'a>(divider: bool, large_size: bool) -> CreateContainerComponent<'a> {
    let mut separator = CreateSeparator::new();

    if !divider {
        separator = separator.divider(false);
    }

    if large_size {
        separator = separator.spacing(SeparatorSpacingSize::Large);
    }

    CreateContainerComponent::Separator(separator)
}

pub fn create_divider<'a>(large_size: bool) -> CreateContainerComponent<'a> {
    _create_separator(true, large_size)
}

pub fn create_separator<'a>(large_size: bool) -> CreateContainerComponent<'a> {
    _create_separator(false, large_size)
}

pub fn create_container_text<'a>(content: impl Into<Cow<'a, str>>) -> CreateContainerComponent<'a> {
    CreateContainerComponent::TextDisplay(CreateTextDisplay::new(content))
}

pub fn create_container_section<'a>(
    components: impl Into<Cow<'a, [CreateSectionComponent<'a>]>>,
    accessory: CreateSectionAccessory<'a>,
) -> CreateContainerComponent<'a> {
    CreateContainerComponent::Section(CreateSection::new(components, accessory))
}

pub fn create_section_text<'a>(content: impl Into<Cow<'a, str>>) -> CreateSectionComponent<'a> {
    CreateSectionComponent::TextDisplay(CreateTextDisplay::new(content))
}

pub fn create_section_thumbnail<'a>(
    url: impl Into<Cow<'a, str>>,
    description: Option<&'a str>,
    spoiler: bool,
) -> CreateSectionAccessory<'a> {
    let mut thumbnail = create_thumbnail(url);

    if let Some(description) = description {
        thumbnail = thumbnail.description(description);
    }

    if spoiler {
        thumbnail = thumbnail.spoiler(spoiler);
    }

    CreateSectionAccessory::Thumbnail(thumbnail)
}
