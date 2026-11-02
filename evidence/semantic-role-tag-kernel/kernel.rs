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
}

pub(crate) fn role_uses_native_button(role:Role)->(result:bool)
    ensures result == (match role {Role::Button=>true,_=>false}),
    {
    matches!(role,Role::Button)
}

/// Only Button selects a different native element tag in append_prepared_subtree.
pub(crate) fn semantic_role_update_preserves_tag(current:Role,next:Role)->(result:bool)
    ensures result == ((match current {Role::Button=>true,_=>false}) == (match next {Role::Button=>true,_=>false})),
    {
    role_uses_native_button(current)==role_uses_native_button(next)
}

}
