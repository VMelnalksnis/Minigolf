use {
    bevy::{prelude::*, reflect::GetTypeRegistration},
    bevy_replicon::{
        bytes::Bytes,
        prelude::*,
        shared::{
            postcard_utils,
            replication::replication_registry::{
                command_fns::MutWrite,
                ctx::{SerializeCtx, WriteCtx},
                rule_fns::RuleFns,
            },
        },
    },
    serde::{Serialize, de::DeserializeOwned},
};

pub(crate) fn register_replicated<
    TComponent: Component + GetTypeRegistration + Serialize + DeserializeOwned,
>(
    app: &mut App,
) where
    <TComponent as bevy::prelude::Component>::Mutability: MutWrite<TComponent>,
{
    app.register_type::<TComponent>();
    app.replicate::<TComponent>();
}

pub(crate) fn get_child_of_serialization_rules() -> RuleFns<ChildOf> {
    RuleFns::new(serialize_child_of, deserialize_child_of)
}

fn serialize_child_of(
    _ctx: &SerializeCtx,
    child_of: &ChildOf,
    message: &mut Vec<u8>,
) -> Result<()> {
    postcard_utils::to_extend_mut(&child_of.parent(), message)?;
    Ok(())
}

fn deserialize_child_of(ctx: &mut WriteCtx, message: &mut Bytes) -> Result<ChildOf> {
    let entity = postcard_utils::from_buf(message)?;
    let mut component = ChildOf(entity);
    ChildOf::map_entities(&mut component, ctx);
    Ok(component)
}
