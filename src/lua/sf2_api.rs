use std::fs;
use std::io;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct LuaApiItem {
    pub name: &'static str,
    pub category: &'static str,
    pub snippet: &'static str,
    pub description: &'static str,
}

pub const SF2_API_DATABASE: &[LuaApiItem] = &[
    // --- TEMPLATES / COOKBOOK ---
    LuaApiItem {
        name: "[Snippet] Interactive Lever Gate",
        category: "Snippets",
        snippet: "-- Put in INIT State:\nMapFlagSetFalse {Name = \"start_gate\"},\n\n-- Put before MAIN State:\nOnLeverEvent\n{\n    Delay = 5,\n    Lever = \"lever_gate_start\",\n    OnConditions = {},\n    Actions = {\n        MapFlagSetTrue {Name = \"start_gate\"},\n    },\n};\n\nOnEvent\n{\n    EventName = \"GateOpenTrigger\",\n    Conditions = {\n        MapFlagIsTrue {Name = \"start_gate\"},\n        GateIsClosed {Tag = \"gate_start\"},\n    },\n    Actions = {\n        GateSetOpen {Tag = \"gate_start\"},\n        MapFlagSetFalse {Name = \"start_gate\"},\n    },\n};\n\nOnEvent\n{\n    EventName = \"GateCloseTrigger\",\n    Conditions = {\n        MapFlagIsTrue {Name = \"start_gate\"},\n        GateIsOpen {Tag = \"gate_start\"},\n    },\n    Actions = {\n        GateSetClosed {Tag = \"gate_start\"},\n        MapFlagSetFalse {Name = \"start_gate\"},\n    },\n};",
        description: "Complete toggle logic for an interactive lever and gate mechanism with terrain path unblocking.",
    },
    LuaApiItem {
        name: "[Snippet] Transfer Unit to Player",
        category: "Snippets",
        snippet: "OnEvent\n{\n    Conditions = {\n        -- Specify condition when the unit should be handed over\n    },\n    Actions = {\n        FigureTeamTransfer {Tag = \"Name_of_the_unit\", Team = \"tm_Human\"},\n        FigurePlayerTransfer {Tag = \"Name_of_the_unit\", Player = \"pl_Human1\"},\n    },\n}",
        description: "Transfers unit team allegiance to tm_Human and assigns direct control to a specific player slot.",
    },
    LuaApiItem {
        name: "[Snippet] Unit Death Trigger",
        category: "Snippets",
        snippet: "OnEvent\n{\n    Conditions = {\n        FigureIsDead {Tag = \"Target_Boss\"},\n    },\n    Actions = {\n        QuestSetSolved {Player = \"pl_Human\", Quest = \"MainQuest_KillBoss\"},\n    },\n}",
        description: "Executes actions immediately when a designated boss or NPC unit dies.",
    },
    LuaApiItem {
        name: "[Snippet] Item Pickup Quest Event",
        category: "Snippets",
        snippet: "OnEvent\n{\n    Conditions = {\n        AvatarHasItemMisc {Player = \"pl_Human\", ItemId = 550, Amount = 1},\n    },\n    Actions = {\n        AvatarItemMiscTake {Player = \"pl_Human\", ItemId = 550, Amount = 1},\n        QuestSetSolved {Player = \"pl_Human\", Quest = \"FindRelic\"},\n    },\n}",
        description: "Triggers when the player collects a specific item, consumes it, and updates quest status.",
    },
    // --- EVENTS & STATES ---
    LuaApiItem {
        name: "OnEvent",
        category: "Events",
        snippet: "OnEvent\n{\n    EventName = \"Event_Name\",  -- Event name (for debugging)\n    Conditions = {},\n    Actions = {},\n    GotoState = \"self\",       -- Target state name\n}",
        description: "Standard state event. Fires continuously while all Conditions remain true.",
    },
    LuaApiItem {
        name: "OnOneTimeEvent",
        category: "Events",
        snippet: "OnOneTimeEvent\n{\n    EventName = \"OneTime_Event\",\n    Conditions = {},\n    Actions = {},\n    GotoState = \"self\",\n}",
        description: "Executes once when conditions are met and is then permanently disabled.",
    },
    LuaApiItem {
        name: "OnLeverEvent",
        category: "Events",
        snippet: "OnLeverEvent\n{\n    Delay = 5,           -- Cooldown in seconds before the lever can be pulled again\n    Lever = \"lever_tag\", -- Script tag of the lever entity\n    OnConditions = {},\n    Actions = {},\n};",
        description: "Fires when a player entity interacts with a map lever.",
    },
    LuaApiItem {
        name: "OnFigureRespawnEvent",
        category: "Events",
        snippet: "OnFigureRespawnEvent\n{\n    WaitTime = 10,        -- Seconds to wait before respawning after death\n    X = [self],           -- Grid X coordinate\n    Y = [self],           -- Grid Y coordinate\n    Conditions = {},\n    Actions = {},\n    DeathActions = {},\n    DelayedActions = {},\n    NoSpawnEffect = false,\n}",
        description: "Manages automatic respawning behavior for an NPC or creature upon death.",
    },
    LuaApiItem {
        name: "State",
        category: "Events",
        snippet: "State\n{\n    StateName = \"MAIN\",\n    -- Place Events here\n};",
        description: "Defines a finite state machine block containing localized event triggers.",
    },
    // --- FIGURE / UNIT ACTIONS ---
    LuaApiItem {
        name: "FigureNpcSpawn",
        category: "Figures",
        snippet: "FigureNpcSpawn\n{\n    Tag = \"npc_tag\",      -- Script tag for the NPC\n    Level = 5,            -- Unit level\n    UnitId = 100,         -- Database Unit ID\n    X = 100,              -- World X coordinate\n    Y = 100,              -- World Y coordinate\n    Team = \"tm_Neutral\",  -- Team name from map editor\n}",
        description: "Spawns a specific hostile, friendly, or neutral NPC at world coordinates.",
    },
    LuaApiItem {
        name: "FigureHeroSpawn",
        category: "Figures",
        snippet: "FigureHeroSpawn\n{\n    Player = \"pl_Human\", -- Player name configured in team setup\n    Tag = \"hero_tag\",      -- Unique script tag\n    X = 100,\n    Y = 100,\n    Direction = 0,         -- Orientation in degrees (0..360)\n}",
        description: "Spawns a hero assigned to a specific player slot.",
    },
    LuaApiItem {
        name: "FigureTeamTransfer",
        category: "Figures",
        snippet: "FigureTeamTransfer\n{\n    Tag = \"unit_tag\",\n    Team = \"tm_Human\",   -- Target team name from editor\n}",
        description: "Changes the team allegiance of a figure.",
    },
    LuaApiItem {
        name: "FigurePlayerTransfer",
        category: "Figures",
        snippet: "FigurePlayerTransfer\n{\n    Tag = \"unit_tag\",\n    Player = \"pl_Human\",\n}",
        description: "Hands over full RTS/RPG unit control to the specified player.",
    },
    LuaApiItem {
        name: "FigureKill",
        category: "Figures",
        snippet: "FigureKill\n{\n    Tag = \"unit_tag\",\n}",
        description: "Instantly kills the target unit without needing combat damage.",
    },
    LuaApiItem {
        name: "FigureTeleport",
        category: "Figures",
        snippet: "FigureTeleport\n{\n    Tag = \"unit_tag\",\n    X = 150,\n    Y = 200,\n}",
        description: "Instantly teleports a figure to target world coordinates.",
    },
    LuaApiItem {
        name: "FigureHealthSet",
        category: "Figures",
        snippet: "FigureHealthSet\n{\n    Tag = \"unit_tag\",\n    Percent = 100,  -- Percentage value (0..100)\n}",
        description: "Sets the health percentage of the specified figure.",
    },
    LuaApiItem {
        name: "FigureCastSpell",
        category: "Figures",
        snippet: "FigureCastSpell\n{\n    Tag = \"caster_tag\",\n    Spell = 50,    -- Spell ID from database\n    Power = 1,     -- Spell power multiplier\n    X = 100,\n    Y = 100,\n}",
        description: "Forces a figure to cast a spell at specific target coordinates.",
    },
    LuaApiItem {
        name: "FigureAttackEntity",
        category: "Figures",
        snippet: "FigureAttackEntity\n{\n    Tag = \"attacker_tag\",\n    TargetTag = \"victim_tag\",\n}",
        description: "Forces a figure to acquire and attack a designated target entity.",
    },
    LuaApiItem {
        name: "FigurePatrolWalk",
        category: "Figures",
        snippet: "FigurePatrolWalk\n{\n    Tag = \"unit_tag\",\n    X = 120,\n    Y = 140,\n}",
        description: "Directs a figure to walk along a patrol route to target coordinates.",
    },
    // --- QUESTS, DIALOGS & CUTSCENES ---
    LuaApiItem {
        name: "QuestSetActive",
        category: "Quests",
        snippet: "QuestSetActive\n{\n    Player = \"pl_Human\",\n    Quest = \"QuestTag\",   -- Quest identifier tag\n}",
        description: "Activates a quest entry in the player quest journal.",
    },
    LuaApiItem {
        name: "QuestSetSolved",
        category: "Quests",
        snippet: "QuestSetSolved\n{\n    Player = \"pl_Human\",\n    Quest = \"QuestTag\",\n}",
        description: "Marks a quest as successfully completed.",
    },
    LuaApiItem {
        name: "FigureOutcry",
        category: "Dialogs",
        snippet: "FigureOutcry\n{\n    TextTag = \"Dialogue_LocaTag\", -- Text localization key\n    Tag = \"speaker_tag\",          -- Unit script tag\n}",
        description: "Displays speech bubble text and triggers voice acting playback for a character.",
    },
    LuaApiItem {
        name: "DialogBegin",
        category: "Dialogs",
        snippet: "DialogBegin\n{\n    Player = \"pl_Human\",\n    Tag = \"npc_tag\",\n}",
        description: "Initiates full RPG conversation mode between the player avatar and target NPC.",
    },
    LuaApiItem {
        name: "CutsceneBegin",
        category: "Dialogs",
        snippet: "CutsceneBegin\n{\n    File = \"cutscene_music\",  -- Music file in sound/00_music without .mp3\n}",
        description: "Disables standard player controls and launches cinematic cutscene camera mode.",
    },
    LuaApiItem {
        name: "CutsceneEnd",
        category: "Dialogs",
        snippet: "CutsceneEnd\n{\n}",
        description: "Terminates the active cutscene and restores gameplay UI controls.",
    },
    // --- WORLD, ENVIRONMENT & OBJECTS ---
    LuaApiItem {
        name: "GateSetOpen",
        category: "World",
        snippet: "GateSetOpen\n{\n    Tag = \"gate_tag\",\n}",
        description: "Opens an interactive gate entity and unblocks navigation mesh terrain.",
    },
    LuaApiItem {
        name: "GateSetClosed",
        category: "World",
        snippet: "GateSetClosed\n{\n    Tag = \"gate_tag\",\n}",
        description: "Closes an interactive gate entity and blocks pathfinding.",
    },
    LuaApiItem {
        name: "WeatherChange",
        category: "World",
        snippet: "WeatherChange\n{\n    File = \"weather_profile\", -- Weather XML file without extension\n    FadeDuration = 3,          -- Transition duration in seconds\n}",
        description: "Smoothly transitions the active ambient weather profile.",
    },
    LuaApiItem {
        name: "FogOfWarReveal",
        category: "World",
        snippet: "FogOfWarReveal\n{\n    FogOfWarId = 0,\n    X = 120,\n    Y = 120,\n    Range = 15,   -- Reveal radius in grid units\n    Height = 3,\n}",
        description: "Permanently clears fog of war visibility at coordinates.",
    },
    // --- AI SCRIPTING ---
    LuaApiItem {
        name: "AIAttackFrequencySet",
        category: "AI",
        snippet: "AIAttackFrequencySet\n{\n    Player = \"pl_AI1\",\n    Minutes = 5,  -- Wave interval in minutes\n}",
        description: "Configures how frequently an AI faction dispatches attack waves.",
    },
    LuaApiItem {
        name: "AILevelSet",
        category: "AI",
        snippet: "AILevelSet\n{\n    Player = \"pl_AI1\",\n    Level = 10,   -- Target difficulty level\n}",
        description: "Sets the unit, building, and research tech level for an AI player.",
    },
    LuaApiItem {
        name: "AIEnemyAdd",
        category: "AI",
        snippet: "AIEnemyAdd\n{\n    Player = \"pl_AI1\",\n    Tag = \"enemy_unit_or_building\",\n}",
        description: "Adds a specific script tag as a priority attack target for an AI faction.",
    },
    // --- CONDITIONS ---
    LuaApiItem {
        name: "FigureIsDead",
        category: "Conditions",
        snippet: "FigureIsDead\n{\n    Tag = \"unit_tag\",\n}",
        description: "Evaluates to true if the designated figure is dead.",
    },
    LuaApiItem {
        name: "FigureIsAlive",
        category: "Conditions",
        snippet: "FigureIsAlive\n{\n    Tag = \"unit_tag\",\n}",
        description: "Evaluates to true if the designated figure is currently alive.",
    },
    LuaApiItem {
        name: "FigureIsInRange",
        category: "Conditions",
        snippet: "FigureIsInRange\n{\n    Tag = \"unit_tag\",\n    Range = 10,\n    X = 100,\n    Y = 100,\n}",
        description: "Checks whether a unit is within N grid units of coordinate X, Y.",
    },
    LuaApiItem {
        name: "AvatarHasItemMisc",
        category: "Conditions",
        snippet: "AvatarHasItemMisc\n{\n    Player = \"pl_Human\",\n    ItemId = 100,\n    Amount = 1,\n}",
        description: "Checks whether the player avatar carries a specific miscellaneous item.",
    },
    LuaApiItem {
        name: "MapFlagIsTrue",
        category: "Conditions",
        snippet: "MapFlagIsTrue\n{\n    Name = \"flag_name\",\n}",
        description: "Evaluates to true if a custom map boolean variable is set to true.",
    },
    LuaApiItem {
        name: "GateIsOpen",
        category: "Conditions",
        snippet: "GateIsOpen\n{\n    Tag = \"gate_tag\",\n}",
        description: "Checks whether a gate entity is currently open.",
    },
    LuaApiItem {
        name: "GateIsClosed",
        category: "Conditions",
        snippet: "GateIsClosed\n{\n    Tag = \"gate_tag\",\n}",
        description: "Checks whether a gate entity is currently closed.",
    },
    LuaApiItem {
        name: "TimeIsBetween",
        category: "Conditions",
        snippet: "TimeIsBetween\n{\n    StartHour = 8.0,\n    EndHour = 20.0,\n}",
        description: "Checks whether current in-game day/night time falls between the specified hours.",
    },
];

pub fn search_api(query: &str, category: &str) -> Vec<&'static LuaApiItem> {
    let q = query.to_lowercase();
    SF2_API_DATABASE
        .iter()
        .filter(|item| {
            let cat_match = category == "All Categories" || item.category == category;
            let query_match = q.is_empty()
                || item.name.to_lowercase().contains(&q)
                || item.snippet.to_lowercase().contains(&q)
                || item.description.to_lowercase().contains(&q);
            cat_match && query_match
        })
        .collect()
}

pub fn export_emmylua_definitions(out_file: &Path) -> io::Result<usize> {
    let mut out = String::new();
    out.push_str(
        "---@meta\n--- SpellForce 2 Lua API EmmyLua Definitions\n--- Generated by SFTool\n\n",
    );

    let mut count = 0;
    for item in SF2_API_DATABASE {
        if item.category == "Snippets" {
            continue;
        }
        out.push_str(&format!("--- {}\n", item.description));
        out.push_str("---@param params table Table containing function arguments\n");
        out.push_str(&format!("function {}(params) end\n\n", item.name));
        count += 1;
    }

    fs::write(out_file, out)?;
    Ok(count)
}
