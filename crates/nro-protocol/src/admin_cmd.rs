//! Strongly typed chat command parser (`Command.java`) for admin and pet chat triggers.

use nro_core::{MapId, TemplateId};

/// Custom `(option_id, param)` modifier parsed from admin `"i <itemId> [qty] [optId:val ...]"` syntax.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdminItemOption {
    /// Item option template ID (`ItemOption.optionTemplate.id`).
    pub option_id: i32,
    /// Option parameter magnitude (`ItemOption.param`).
    pub param: i32,
}

/// Syntax errors matching exact Vietnamese feedback strings emitted by `Command.java`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AdminCommandError {
    /// `"Sai định dạng map ID!"` (`Command.java:76`).
    #[error("Sai định dạng map ID!")]
    InvalidMapId,
    /// `"Sai định dạng task ID!"` (`Command.java:135`).
    #[error("Sai định dạng task ID!")]
    InvalidTaskId,
    /// `"Sai cú pháp: dmg <số>"` (`Command.java:146`).
    #[error("Sai cú pháp: dmg <số>")]
    InvalidDamage,
    /// `"Sai cú pháp: hpg <số>"` (`Command.java:158`).
    #[error("Sai cú pháp: hpg <số>")]
    InvalidBaseHp,
    /// `"Sai cú pháp: ki <số>"` (`Command.java:170`).
    #[error("Sai cú pháp: ki <số>")]
    InvalidBaseKi,
    /// `"Sai cú pháp: up <số>"` (`Command.java:180`).
    #[error("Sai cú pháp: up <số>")]
    InvalidPlayerPower,
    /// `"Sai cú pháp: upp <số>"` (`Command.java:197`).
    #[error("Sai cú pháp: upp <số>")]
    InvalidPetPower,
    /// `"Cú pháp: i <itemId> <số lượng> [option:value...]"` (`Command.java:206`).
    #[error("Cú pháp: i <itemId> <số lượng> [option:value...]")]
    MissingSpawnItemArgs,
    /// `"Lỗi cú pháp! Dùng: i <itemId> <số lượng> [optionId:value]"` (`Command.java:240`).
    #[error("Lỗi cú pháp! Dùng: i <itemId> <số lượng> [optionId:value]")]
    InvalidSpawnItemSyntax,
}

/// Strongly typed admin command covering all exact and prefix commands in `Command.java`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdminCommand {
    /// `"item"`: Open give-item-to-player form (`Input.createFormGiveItem`).
    OpenGiveItemForm,
    /// `"brl"`: Show active Broly boss list (`BrolyManager.showListBoss`).
    ShowBrolyBossList,
    /// `"getitem"`: Open quick self-item form (`Input.createFormGetItem`).
    OpenGetItemForm,
    /// `"hs"`: Release all skill cooldowns (`Service.releaseCooldownSkill`).
    ReleaseSkillCooldowns,
    /// `"d"`: Nudge player down by `+10` pixels (`Service.setPos(x, y + 10)`).
    NudgeDown {
        /// Vertical offset in pixels (`10`).
        delta_y: i16,
    },
    /// `"a"`: Open `ConstNpc.MENU_ADMIN` (`512`) server telemetry dashboard.
    OpenAdminDashboard,
    /// `"toado"`: Send OK dialog displaying current `x` and `y` coordinates.
    ShowCoordinates,
    /// `"upp <power>"`: Add `power` SM/TN to `player.pet` (ordered before `"up"`!).
    AddPetPower {
        /// Power/potential delta (`i64`).
        power: i64,
    },
    /// `"up <power>"`: Add `power` SM/TN to `player`.
    AddPlayerPower {
        /// Power/potential delta (`i64`).
        power: i64,
    },
    /// `"dmg <damage>"` / `"dm <damage>"`: Set `player.nPoint.dameg = damage`.
    SetBaseDamage {
        /// New base damage (`i32`).
        damage: i32,
    },
    /// `"hpg <hpg>"` / `"hp <hpg>"`: Set `player.nPoint.hpg = hpg`.
    SetBaseHp {
        /// New base HP (`i32`).
        hp_base: i32,
    },
    /// `"ki <mpg>"`: Set `player.nPoint.mpg = mpg`.
    SetBaseKi {
        /// New base Ki/MP (`i32`).
        mp_base: i32,
    },
    /// `"i <itemId> [qty] [optId:val ...]"`: Spawn item(s) with optional custom `ItemOption`s.
    SpawnItems {
        /// Item template ID (`short`).
        item_id: TemplateId,
        /// Number of item instances to add (`default = 1`).
        quantity: i32,
        /// Custom `(option_id, param)` list; if empty, use default shop options.
        options: Vec<AdminItemOption>,
    },
    /// `"m <mapId>"` / `"m<mapId>"`: Teleport admin into `map_id` via `changeMapInYard`.
    TeleportMap {
        /// Destination map ID (`MapId`).
        map_id: MapId,
    },
    /// `"n <taskId>"` / `"n<taskId>"`: Jump main quest to `task_id`.
    SetMainTask {
        /// Target task ID (`taskMain.id` is set to `task_id - 1` before `sendNextTaskMain`).
        task_id: i32,
    },
    /// `"1"`: Open Bot Manager menu (`npc_id = 206_783`).
    OpenBotMenu,
    /// `"2"`: Switch to `PK_ALL` and spawn a `BotAttackplayer` clone at current coordinates.
    SpawnAttackBot,
    /// `"b"`: Open custom multi-option item sender form (`Input.createFormSenditem1`).
    OpenSendItemForm1,
}

/// Non-admin pet chat triggers in `Command.java` (lines 270–311).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PetChatCommand {
    /// `"ten con la <new_name>"`: Rename pet (`PetService.changeNamePet`).
    Rename {
        /// New pet name stripped of `"ten con la "`.
        new_name: String,
    },
    /// `"di theo"` | `"follow"`: Set pet status to `Pet.FOLLOW` (`0`).
    Follow,
    /// `"bao ve"` | `"protect"`: Set pet status to `Pet.PROTECT` (`1`).
    Protect,
    /// `"tan cong"` | `"attack"`: Set pet status to `Pet.ATTACK` (`2`).
    Attack,
    /// `"ve nha"` | `"go home"`: Set pet status to `Pet.GOHOME` (`3`).
    GoHome,
    /// `"bien hinh"`: Trigger pet transformation (`player.pet.transform()`).
    Transform,
    /// `"sach tuyet ky"`: Equip first bag item with `template.type == 25` onto eligible pet.
    EquipSkillBook,
}

/// Complete outcome of `Command.chat(player, text)` (`Command.java:245-313`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatDirective {
    /// Whitespace-only input; drop silently without broadcasting (`cleanedText.isEmpty()`).
    EmptyIgnore,
    /// Admin command matched (either valid or syntax error toast); **suppresses** public zone chat.
    Admin(Result<AdminCommand, AdminCommandError>),
    /// Normal zone chat broadcast, optionally paired with a pet command side effect.
    ZoneChat {
        /// Trimmed chat string to broadcast via `Service.chat(player, cleaned_text)`.
        cleaned_text: String,
        /// Optional pet action triggered alongside the spoken chat bubble.
        pet_side_effect: Option<PetChatCommand>,
    },
}

/// Parses raw `cmd 44` chat text into a strongly typed [`ChatDirective`].
#[must_use]
pub fn parse_chat_command(raw_text: &str, is_admin: bool) -> ChatDirective {
    let cleaned = raw_text.trim();
    if cleaned.is_empty() {
        return ChatDirective::EmptyIgnore;
    }
    if let Some(pet_cmd) = parse_pet_trigger(cleaned) {
        return ChatDirective::ZoneChat {
            cleaned_text: cleaned.to_owned(),
            pet_side_effect: Some(pet_cmd),
        };
    }
    if is_admin && let Some(admin_outcome) = try_parse_admin(cleaned) {
        return ChatDirective::Admin(admin_outcome);
    }
    ChatDirective::ZoneChat {
        cleaned_text: cleaned.to_owned(),
        pet_side_effect: None,
    }
}

fn strip_num_cmd<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let rest = text.strip_prefix(prefix)?;
    if rest.is_empty()
        || rest.starts_with(char::is_whitespace)
        || rest.starts_with(|c: char| c.is_ascii_digit() || c == '-')
    {
        Some(rest.trim())
    } else {
        None
    }
}

fn try_parse_admin(text: &str) -> Option<Result<AdminCommand, AdminCommandError>> {
    match text {
        "item" => return Some(Ok(AdminCommand::OpenGiveItemForm)),
        "brl" => return Some(Ok(AdminCommand::ShowBrolyBossList)),
        "getitem" => return Some(Ok(AdminCommand::OpenGetItemForm)),
        "hs" => return Some(Ok(AdminCommand::ReleaseSkillCooldowns)),
        "d" => return Some(Ok(AdminCommand::NudgeDown { delta_y: 10 })),
        "a" => return Some(Ok(AdminCommand::OpenAdminDashboard)),
        "1" => return Some(Ok(AdminCommand::OpenBotMenu)),
        "2" => return Some(Ok(AdminCommand::SpawnAttackBot)),
        "b" => return Some(Ok(AdminCommand::OpenSendItemForm1)),
        _ => {}
    }
    if text == "toado"
        || text
            .strip_prefix("toado")
            .is_some_and(|r| r.starts_with(char::is_whitespace))
    {
        return Some(Ok(AdminCommand::ShowCoordinates));
    }
    if let Some(rest) = strip_num_cmd(text, "upp") {
        return Some(
            rest.parse::<i64>()
                .map(|power| AdminCommand::AddPetPower { power })
                .map_err(|_| AdminCommandError::InvalidPetPower),
        );
    }
    if let Some(rest) = strip_num_cmd(text, "up") {
        return Some(
            rest.parse::<i64>()
                .map(|power| AdminCommand::AddPlayerPower { power })
                .map_err(|_| AdminCommandError::InvalidPlayerPower),
        );
    }
    if let Some(rest) = strip_num_cmd(text, "dmg").or_else(|| strip_num_cmd(text, "dm")) {
        return Some(
            rest.parse::<i32>()
                .map(|damage| AdminCommand::SetBaseDamage { damage })
                .map_err(|_| AdminCommandError::InvalidDamage),
        );
    }
    if let Some(rest) = strip_num_cmd(text, "hpg").or_else(|| strip_num_cmd(text, "hp")) {
        return Some(
            rest.parse::<i32>()
                .map(|hp_base| AdminCommand::SetBaseHp { hp_base })
                .map_err(|_| AdminCommandError::InvalidBaseHp),
        );
    }
    if let Some(rest) = strip_num_cmd(text, "ki") {
        return Some(
            rest.parse::<i32>()
                .map(|mp_base| AdminCommand::SetBaseKi { mp_base })
                .map_err(|_| AdminCommandError::InvalidBaseKi),
        );
    }
    if text == "i"
        || text
            .strip_prefix('i')
            .is_some_and(|r| r.starts_with(char::is_whitespace))
    {
        return Some(parse_spawn_items(text));
    }
    if let Some(rest) = strip_num_cmd(text, "m") {
        return Some(
            rest.parse::<i16>()
                .map(|id| AdminCommand::TeleportMap { map_id: MapId(id) })
                .map_err(|_| AdminCommandError::InvalidMapId),
        );
    }
    if let Some(rest) = strip_num_cmd(text, "n") {
        return Some(
            rest.parse::<i32>()
                .map(|task_id| AdminCommand::SetMainTask { task_id })
                .map_err(|_| AdminCommandError::InvalidTaskId),
        );
    }
    None
}

fn parse_spawn_items(text: &str) -> Result<AdminCommand, AdminCommandError> {
    let parts: Vec<&str> = text.split_whitespace().collect();
    if parts.len() < 2 {
        return Err(AdminCommandError::MissingSpawnItemArgs);
    }
    let item_id = parts[1]
        .parse::<i32>()
        .ok()
        .and_then(|v| i16::try_from(v).ok())
        .map(TemplateId)
        .ok_or(AdminCommandError::InvalidSpawnItemSyntax)?;
    let quantity = if parts.len() >= 3 {
        parts[2]
            .parse::<i32>()
            .map_err(|_| AdminCommandError::InvalidSpawnItemSyntax)?
    } else {
        1
    };
    if quantity <= 0 {
        return Err(AdminCommandError::InvalidSpawnItemSyntax);
    }
    let mut options = Vec::new();
    for token in parts.iter().skip(3) {
        let (opt_str, val_str) = token
            .split_once(':')
            .ok_or(AdminCommandError::InvalidSpawnItemSyntax)?;
        let option_id = opt_str
            .parse::<i32>()
            .map_err(|_| AdminCommandError::InvalidSpawnItemSyntax)?;
        let param = val_str
            .parse::<i32>()
            .map_err(|_| AdminCommandError::InvalidSpawnItemSyntax)?;
        options.push(AdminItemOption { option_id, param });
    }
    Ok(AdminCommand::SpawnItems {
        item_id,
        quantity,
        options,
    })
}

fn parse_pet_trigger(text: &str) -> Option<PetChatCommand> {
    if let Some(name) = text.strip_prefix("ten con la ") {
        return Some(PetChatCommand::Rename {
            new_name: name.to_owned(),
        });
    }
    match text {
        "di theo" | "follow" => Some(PetChatCommand::Follow),
        "bao ve" | "protect" => Some(PetChatCommand::Protect),
        "tan cong" | "attack" => Some(PetChatCommand::Attack),
        "ve nha" | "go home" => Some(PetChatCommand::GoHome),
        "bien hinh" => Some(PetChatCommand::Transform),
        "sach tuyet ky" => Some(PetChatCommand::EquipSkillBook),
        _ => None,
    }
}

#[cfg(test)]
#[path = "admin_cmd_tests.rs"]
mod tests;
