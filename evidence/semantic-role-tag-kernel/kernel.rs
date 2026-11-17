use vstd::prelude::*;
verus! {



#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Role {
    #[default]
    Container,
    Text,
    Button,
    TextField,
    Checkbox,
    Radio,
    Switch,
    Slider,
    Image,
    Link,
    List,
    ListItem,
    Dialog,
    Menu,
    MenuItem,
    ScrollView,
    Unknown,
    /// Section heading; the level is 1..=6 by construction.
    /// 章节标题；级别由构造保证为 1..=6。
    Heading(HeadingLevel),
    /// Advisory status output (implicitly a polite live region).
    /// 状态信息（隐含礼貌型实时区域）。
    Status,
    /// Live region announcing its changes with the given politeness.
    /// 以指定礼貌级别播报变化的实时区域。
    Live(LivePoliteness),
    /// Important, time-sensitive message (implicitly an assertive live region).
    /// 重要且有时效的消息（隐含强制型实时区域）。
    Alert,
    /// Form landmark grouping input controls.
    /// 表单地标，组合输入控件。
    Form,
}

/// Heading level, always within 1..=6; build with `HeadingLevel::new`.
/// 标题级别，恒在 1..=6；经 `HeadingLevel::new` 构造。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HeadingLevel(u8);

/// How assertively a live region's changes are announced.
/// 实时区域变化的播报强度。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LivePoliteness {
    Polite,
    Assertive,
}

pub(crate) fn role_uses_native_button(role:Role)->(result:bool)
    ensures result == (match role {Role::Button=>true,_=>false}),
    {
    matches!(role, Role::Button)
}

/// Only Button selects a different native element tag in append_prepared_subtree.
pub(crate) fn semantic_role_update_preserves_tag(current:Role,next:Role)->(result:bool)
    ensures result == ((match current {Role::Button=>true,_=>false}) == (match next {Role::Button=>true,_=>false})),
    {
    role_uses_native_button(current) == role_uses_native_button(next)
}

}
