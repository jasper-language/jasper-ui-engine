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

/// Only Button selects a different native element tag in append_prepared_subtree.
pub(crate) fn semantic_role_update_preserves_tag(current:Role,next:Role)->(result:bool)
    ensures result == ((match current {Role::Button=>true,_=>false}) == (match next {Role::Button=>true,_=>false})),
    {
    match (current,next) {
        (Role::Button,Role::Button)=>true,
        (Role::Button,_)|(_,Role::Button)=>true,
        _=>true,
    }
}

}
