use super::*;

#[test]
fn pet_triggers_and_normal_chat_are_not_hijacked_by_admin_prefixes() {
    assert_eq!(
        parse_chat_command("bao ve", true),
        ChatDirective::ZoneChat {
            cleaned_text: "bao ve".to_owned(),
            pet_side_effect: Some(PetChatCommand::Protect)
        }
    );
    assert_eq!(
        parse_chat_command("bien hinh", true),
        ChatDirective::ZoneChat {
            cleaned_text: "bien hinh".to_owned(),
            pet_side_effect: Some(PetChatCommand::Transform)
        }
    );
    for chat in [
        "ban do",
        "100tr",
        "200k",
        "mai gap",
        "ngon qua",
        "update server",
        "kiem tra",
    ] {
        assert_eq!(
            parse_chat_command(chat, true),
            ChatDirective::ZoneChat {
                cleaned_text: chat.to_owned(),
                pet_side_effect: None
            }
        );
    }
    assert_eq!(
        parse_chat_command("b", true),
        ChatDirective::Admin(Ok(AdminCommand::OpenSendItemForm1))
    );
    assert_eq!(
        parse_chat_command("1", true),
        ChatDirective::Admin(Ok(AdminCommand::OpenBotMenu))
    );
    assert_eq!(
        parse_chat_command("2", true),
        ChatDirective::Admin(Ok(AdminCommand::SpawnAttackBot))
    );
    assert_eq!(
        parse_chat_command("dmg 250", true),
        ChatDirective::Admin(Ok(AdminCommand::SetBaseDamage { damage: 250 }))
    );
    assert_eq!(
        parse_chat_command("dm 250", true),
        ChatDirective::Admin(Ok(AdminCommand::SetBaseDamage { damage: 250 }))
    );
    assert_eq!(
        parse_chat_command("hpg 1000", true),
        ChatDirective::Admin(Ok(AdminCommand::SetBaseHp { hp_base: 1000 }))
    );
    assert_eq!(
        parse_chat_command("hp 1000", true),
        ChatDirective::Admin(Ok(AdminCommand::SetBaseHp { hp_base: 1000 }))
    );
    assert_eq!(
        parse_chat_command("m5", true),
        ChatDirective::Admin(Ok(AdminCommand::TeleportMap { map_id: MapId(5) }))
    );
    assert_eq!(
        parse_chat_command("i   457   2   30:1", true),
        ChatDirective::Admin(Ok(AdminCommand::SpawnItems {
            item_id: TemplateId(457),
            quantity: 2,
            options: vec![AdminItemOption {
                option_id: 30,
                param: 1
            }],
        }))
    );
    assert_eq!(
        parse_chat_command("i 457 0", true),
        ChatDirective::Admin(Err(AdminCommandError::InvalidSpawnItemSyntax))
    );
}
