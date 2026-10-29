// @generated from the recovered protobuf schemas. Do not edit. Regenerate: (config step).
#![allow(clippy::all, missing_docs)]

use std::collections::BTreeMap;

use serde::Deserialize;
use serde::Serialize;

use super::types::{I64, U64};
use crate::config_enum;

config_enum! { /// `tgf.DSettleInterFaceType`
    DSettleInterFaceType {
        None = 0 => "EN_SETTLE_INTERFACE_TYPE_NONE",
        Pve = 1 => "EN_SETTLE_INTERFACE_TYPE_PVE",
        Pvp = 2 => "EN_SETTLE_INTERFACE_TYPE_PVP",
        Bo5 = 3 => "EN_SETTLE_INTERFACE_TYPE_BO5",
        Bo5Trial = 4 => "EN_SETTLE_INTERFACE_TYPE_BO5_TRIAL",
    }
}

config_enum! { /// `tgf.DStatisticsPVPMode`
    DStatisticsPVPMode {
        None = 0 => "EN_STATISTICS_PVP_MODE_NONE",
        Death = 1 => "EN_STATISTICS_PVP_MODE_DEATH",
        Bo5 = 2 => "EN_STATISTICS_PVP_MODE_BO5",
        Bo7 = 3 => "EN_STATISTICS_PVP_MODE_BO7",
    }
}

config_enum! { /// `tgf.ENRankScoreSettleType`
    ENRankScoreSettleType {
        None = 0 => "EN_RANK_SCORE_SETTLE_NONE",
        SubScore = 1 => "EN_RANK_SCORE_SETTLE_SUB_SCORE",
        ResetScore = 2 => "EN_RANK_SCORE_SETTLE_RESET_SCORE",
        RecentBattleCount = 3 => "EN_RANK_SCORE_SETTLE_RECENT_BATTLE_COUNT",
    }
}

config_enum! { /// `tgf.EnActivityType`
    EnActivityType {
        Default = 0 => "EN_ACTIVITY_CENTER_DEFAULT",
        CumulativeLogin = 1 => "EN_ACTIVITY_CENTER_CUMULATIVE_LOGIN",
        CommonDaily = 2 => "EN_ACTIVITY_CENTER_COMMON_DAILY",
        RandomItem = 3 => "EN_ACTIVITY_CENTER_RANDOM_ITEM",
        AddUpX = 4 => "EN_ACTIVITY_CENTER_ADD_UP_X",
        PicTemplate = 5 => "EN_ACTIVITY_CENTER_PIC_TEMPLATE",
        LevelRefreshConfig = 6 => "EN_ACTIVITY_CENTER_LEVEL_REFRESH_CONFIG",
        OnlyShow = 7 => "EN_ACTIVITY_CENTER_ONLY_SHOW",
        SteamAchievement = 8 => "EN_ACTIVITY_CENTER_STEAM_ACHIEVEMENT",
        Passport = 9 => "EN_ACTIVITY_CENTER_PASSPORT",
        ImmaculateTrial = 10 => "EN_ACTIVITY_CENTER_IMMACULATE_TRIAL",
    }
}

config_enum! { /// `tgf.EnAffixCorruptionGroup`
    EnAffixCorruptionGroup {
        Invalid = 0 => "EN_AFFIX_CORRUPTION_GROUP_INVALID",
        EnhancedSingle = 1 => "EN_AFFIX_CORRUPTION_GROUP_ENHANCED_SINGLE",
        EnhancedBoth = 2 => "EN_AFFIX_CORRUPTION_GROUP_ENHANCED_BOTH",
        EpicSingle = 3 => "EN_AFFIX_CORRUPTION_GROUP_EPIC_SINGLE",
        EpicBoth = 4 => "EN_AFFIX_CORRUPTION_GROUP_EPIC_BOTH",
    }
}

config_enum! { /// `tgf.EnAffixCorruptionType`
    EnAffixCorruptionType {
        Invalid = 0 => "EN_AFFIX_CORRUPTION_TYPE_INVALID",
        V1 = 1 => "EN_AFFIX_CORRUPTION_TYPE_1",
        V2 = 2 => "EN_AFFIX_CORRUPTION_TYPE_2",
        V3 = 3 => "EN_AFFIX_CORRUPTION_TYPE_3",
    }
}

config_enum! { /// `tgf.EnBattleAffixEffectiveLocation`
    EnBattleAffixEffectiveLocation {
        None = 0 => "EN_BATTLE_AFFIX_EFFECTIVE_LOCATION_NONE",
        RoleEquip = 1 => "EN_BATTLE_AFFIX_EFFECTIVE_LOCATION_ROLE_EQUIP",
        RoleInventory = 2 => "EN_BATTLE_AFFIX_EFFECTIVE_LOCATION_ROLE_INVENTORY",
        RoleHold = 3 => "EN_BATTLE_AFFIX_EFFECTIVE_LOCATION_ROLE_HOLD",
    }
}

config_enum! { /// `tgf.EnBattleAffixType`
    EnBattleAffixType {
        Default = 0 => "EN_BATTLE_AFFIX_TYPE_DEFAULT",
        Basic = 5 => "EN_BATTLE_AFFIX_TYPE_BASIC",
        Major = 10 => "EN_BATTLE_AFFIX_TYPE_MAJOR",
        Minor = 20 => "EN_BATTLE_AFFIX_TYPE_MINOR",
        Legendary = 30 => "EN_BATTLE_AFFIX_TYPE_LEGENDARY",
    }
}

config_enum! { /// `tgf.EnBattleGameplayPlayerSceneTag`
    EnBattleGameplayPlayerSceneTag {
        Default = 0 => "EN_BATTLE_GAMEPLAY_PLAYER_SCENE_TAG_DEFAULT",
        Shadow = 1 => "EN_BATTLE_GAMEPLAY_PLAYER_SCENE_TAG_SHADOW",
    }
}

config_enum! { /// `tgf.config.EnDLCType`
    EnDLCType {
        Unknow = 0 => "EN_DLC_TYPE_UNKNOW",
        Packet = 1 => "EN_DLC_TYPE_PACKET",
    }
}

config_enum! { /// `tgf.EnDSPlayerActorStatusType`
    EnDSPlayerActorStatusType {
        Unknow = 0 => "EN_DS_PLAYER_ACTOR_STATUS_TYPE_UNKNOW",
        SelfQuit = 1 => "EN_DS_PLAYER_ACTOR_STATUS_TYPE_SELF_QUIT",
        Dead = 2 => "EN_DS_PLAYER_ACTOR_STATUS_TYPE_DEAD",
        Escape = 3 => "EN_DS_PLAYER_ACTOR_STATUS_TYPE_ESCAPE",
        Alive = 4 => "EN_DS_PLAYER_ACTOR_STATUS_TYPE_ALIVE",
        DsEndButNotReceiveGameOver = 11 => "EN_DS_PLAYER_ACTOR_STATUS_TYPE_DS_END_BUT_NOT_RECEIVE_GAME_OVER",
        DsStatusError = 12 => "EN_DS_PLAYER_ACTOR_STATUS_TYPE_DS_STATUS_ERROR",
        DsStatusBusinessError = 13 => "EN_DS_PLAYER_ACTOR_STATUS_TYPE_DS_STATUS_BUSINESS_ERROR",
        LobbySelfQuit = 14 => "EN_DS_PLAYER_ACTOR_STATUS_TYPE_LOBBY_SELF_QUIT",
        ThisPlayerNotFoundInDs = 15 => "EN_DS_PLAYER_ACTOR_STATUS_TYPE_THIS_PLAYER_NOT_FOUND_IN_DS",
        KickOffAndGameOverAtOnce = 16 => "EN_DS_PLAYER_ACTOR_STATUS_TYPE_KICK_OFF_AND_GAME_OVER_AT_ONCE",
        LobbySelfQuitWhenBattleSvrRecoverError = 17 => "EN_DS_PLAYER_ACTOR_STATUS_TYPE_LOBBY_SELF_QUIT_WHEN_BATTLE_SVR_RECOVER_ERROR",
        LobbySelfQuitDsGameoverForIgnore = 18 => "EN_DS_PLAYER_ACTOR_STATUS_TYPE_LOBBY_SELF_QUIT_DS_GAMEOVER_FOR_IGNORE",
        LobbySelfQuitByClientVersionChange = 19 => "EN_DS_PLAYER_ACTOR_STATUS_TYPE_LOBBY_SELF_QUIT_BY_CLIENT_VERSION_CHANGE",
        DsEndUnexpected = 20 => "EN_DS_PLAYER_ACTOR_STATUS_TYPE_DS_END_UNEXPECTED",
    }
}

config_enum! { /// `tgf.EnDsBattleScenesType`
    EnDsBattleScenesType {
        Invalid = 0 => "EN_DS_BATTLE_SCENES_TYPE_INVALID",
        WaitingRoom = 1 => "EN_DS_BATTLE_SCENES_TYPE_WAITING_ROOM",
        BattleRoomLevelOne = 2 => "EN_DS_BATTLE_SCENES_TYPE_BATTLE_ROOM_LEVEL_ONE",
        BattleRoomLevelTwo = 3 => "EN_DS_BATTLE_SCENES_TYPE_BATTLE_ROOM_LEVEL_TWO",
        BattleRoomLevelTwoPurpleDoor = 4 => "EN_DS_BATTLE_SCENES_TYPE_BATTLE_ROOM_LEVEL_TWO_PURPLE_DOOR",
    }
}

config_enum! { /// `tgf.EnEnvironmentType`
    EnEnvironmentType {
        Unavailable = 0 => "EN_ENVIRONMENT_TYPE_UNAVAILABLE",
        AllAvailable = 1 => "EN_ENVIRONMENT_TYPE_ALL_AVAILABLE",
        OnlyEdit = 2 => "EN_ENVIRONMENT_TYPE_ONLY_EDIT",
    }
}

config_enum! { /// `tgf.config.EnErrorCodePopupMode`
    EnErrorCodePopupMode {
        Default = 0 => "EN_ERROR_CODE_POPUP_MODE_DEFAULT",
        Modal = 1 => "EN_ERROR_CODE_POPUP_MODE_MODAL",
    }
}

config_enum! { /// `tgf.EnFacadeAccessType`
    EnFacadeAccessType {
        Market = 0 => "EN_FACADE_SALES_TYPE_MARKET",
        SteamDlc = 1 => "EN_FACADE_SALES_TYPE_STEAM_DLC",
        Activity = 2 => "EN_FACADE_SALES_TYPE_ACTIVITY",
        Quest = 3 => "EN_FACADE_SALES_TYPE_QUEST",
    }
}

config_enum! { /// `tgf.EnFacadeSkinFirstSortType`
    EnFacadeSkinFirstSortType {
        Invalid = 0 => "EN_FACADE_SKIN_FIRST_SORT_INVALID",
        Skin = 1 => "EN_FACADE_SKIN_FIRST_SORT_SKIN",
        KillEffect = 2 => "EN_FACADE_SKIN_FIRST_SORT_KILL_EFFECT",
    }
}

config_enum! { /// `tgf.EnFacadeSkinSecondSortType`
    EnFacadeSkinSecondSortType {
        Invalid = 0 => "EN_FACADE_SKIN_SECOND_SORT_INVALID",
        Item = 10 => "EN_FACADE_SKIN_SECOND_SORT_ITEM",
        Armoe = 20 => "EN_FACADE_SKIN_SECOND_SORT_ARMOE",
        Heirloom = 30 => "EN_FACADE_SKIN_SECOND_SORT_HEIRLOOM",
        KillEffect = 40 => "EN_FACADE_SKIN_SECOND_SORT_KILL_EFFECT",
        Weapon = 50 => "EN_FACADE_SKIN_SECOND_SORT_WEAPON",
    }
}

config_enum! { /// `tgf.EnFacadeSkinThirdSortType`
    EnFacadeSkinThirdSortType {
        Invalid = 0 => "EN_FACADE_SKIN_THIRD_SORT_INVALID",
        Torch = 101 => "EN_FACADE_SKIN_THIRD_SORT_TORCH",
        Potion = 102 => "EN_FACADE_SKIN_THIRD_SORT_POTION",
        Helmet = 201 => "EN_FACADE_SKIN_THIRD_SORT_HELMET",
        Chest = 202 => "EN_FACADE_SKIN_THIRD_SORT_CHEST",
        Glove = 203 => "EN_FACADE_SKIN_THIRD_SORT_GLOVE",
        Trousers = 204 => "EN_FACADE_SKIN_THIRD_SORT_TROUSERS",
        Shoe = 205 => "EN_FACADE_SKIN_THIRD_SORT_SHOE",
        Cloak = 206 => "EN_FACADE_SKIN_THIRD_SORT_CLOAK",
        Back = 207 => "EN_FACADE_SKIN_THIRD_SORT_BACK",
        HeirloomShandedSword1 = 301 => "EN_FACADE_SKIN_THIRD_SORT_HEIRLOOM_SHANDED_SWORD1",
        HeirloomBhandedSword = 302 => "EN_FACADE_SKIN_THIRD_SORT_HEIRLOOM_BHANDED_SWORD",
        HeirloomShield = 303 => "EN_FACADE_SKIN_THIRD_SORT_HEIRLOOM_SHIELD",
        HeirloomArbalest = 304 => "EN_FACADE_SKIN_THIRD_SORT_HEIRLOOM_ARBALEST",
        HeirloomDagger = 305 => "EN_FACADE_SKIN_THIRD_SORT_HEIRLOOM_DAGGER",
        HeirloomShandedHammer = 306 => "EN_FACADE_SKIN_THIRD_SORT_HEIRLOOM_SHANDED_HAMMER",
        HeirloomFireStaff = 307 => "EN_FACADE_SKIN_THIRD_SORT_HEIRLOOM_FIRE_STAFF",
        HeirloomThunderStaff = 308 => "EN_FACADE_SKIN_THIRD_SORT_HEIRLOOM_THUNDER_STAFF",
        HeirloomArtifacts = 309 => "EN_FACADE_SKIN_THIRD_SORT_HEIRLOOM_ARTIFACTS",
        HeirloomStaff = 310 => "EN_FACADE_SKIN_THIRD_SORT_HEIRLOOM_STAFF",
        KillEffect = 401 => "EN_FACADE_SKIN_THIRD_SORT_KILL_EFFECT",
        ShandedSword = 501 => "EN_FACADE_SKIN_THIRD_SORT_SHANDED_SWORD",
        BhandedSword = 502 => "EN_FACADE_SKIN_THIRD_SORT_BHANDED_SWORD",
        Shield = 503 => "EN_FACADE_SKIN_THIRD_SORT_SHIELD",
        Arbalest = 504 => "EN_FACADE_SKIN_THIRD_SORT_ARBALEST",
        Dagger = 505 => "EN_FACADE_SKIN_THIRD_SORT_DAGGER",
        ShandedHammer = 506 => "EN_FACADE_SKIN_THIRD_SORT_SHANDED_HAMMER",
        FireStaff = 507 => "EN_FACADE_SKIN_THIRD_SORT_FIRE_STAFF",
        ThunderStaff = 508 => "EN_FACADE_SKIN_THIRD_SORT_THUNDER_STAFF",
        Artifacts = 509 => "EN_FACADE_SKIN_THIRD_SORT_ARTIFACTS",
        Staff = 510 => "EN_FACADE_SKIN_THIRD_SORT_STAFF",
    }
}

config_enum! { /// `tgf.EnFacadeType`
    EnFacadeType {
        Undefine = 0 => "EN_FACADE_TYPE_UNDEFINE",
        Avatar = 1 => "EN_FACADE_TYPE_AVATAR",
        AvatarBorder = 2 => "EN_FACADE_TYPE_AVATAR_BORDER",
        Title = 3 => "EN_FACADE_TYPE_TITLE",
        Skin = 4 => "EN_FACADE_TYPE_SKIN",
    }
}

config_enum! { /// `tgf.EnForceType`
    EnForceType {
        Undefine = 0 => "EN_FORCE_TYPE_UNDEFINE",
        Adventurer = 1 => "EN_FORCE_TYPE_ADVENTURER",
        MithrilForce = 2 => "EN_FORCE_TYPE_MITHRIL_FORCE",
    }
}

config_enum! { /// `tgf.EnGearElement`
    EnGearElement {
        None = 0 => "EN_GEAR_ELEMENT_NONE",
        Fire = 1 => "EN_GEAR_ELEMENT_FIRE",
        Ice = 2 => "EN_GEAR_ELEMENT_ICE",
        Thunder = 3 => "EN_GEAR_ELEMENT_THUNDER",
        Light = 4 => "EN_GEAR_ELEMENT_LIGHT",
        Dark = 5 => "EN_GEAR_ELEMENT_DARK",
    }
}

config_enum! { /// `tgf.EnGearEquipSubType`
    EnGearEquipSubType {
        None = 0 => "EN_GEAR_EQUIP_SUBTYPE_NONE",
        ClothChest = 1 => "EN_GEAR_EQUIP_SUBTYPE_CLOTH_CHEST",
        ClothTrousers = 2 => "EN_GEAR_EQUIP_SUBTYPE_CLOTH_TROUSERS",
        ClothHelmet = 3 => "EN_GEAR_EQUIP_SUBTYPE_CLOTH_HELMET",
        ClothShoe = 4 => "EN_GEAR_EQUIP_SUBTYPE_CLOTH_SHOE",
        ClothGlove = 5 => "EN_GEAR_EQUIP_SUBTYPE_CLOTH_GLOVE",
        LeatherChest = 11 => "EN_GEAR_EQUIP_SUBTYPE_LEATHER_CHEST",
        LeatherTrousers = 12 => "EN_GEAR_EQUIP_SUBTYPE_LEATHER_TROUSERS",
        LeatherHelmet = 13 => "EN_GEAR_EQUIP_SUBTYPE_LEATHER_HELMET",
        LeatherShoe = 14 => "EN_GEAR_EQUIP_SUBTYPE_LEATHER_SHOE",
        LeatherGlove = 15 => "EN_GEAR_EQUIP_SUBTYPE_LEATHER_GLOVE",
        PlateChest = 21 => "EN_GEAR_EQUIP_SUBTYPE_PLATE_CHEST",
        PlateTrousers = 22 => "EN_GEAR_EQUIP_SUBTYPE_PLATE_TROUSERS",
        PlateHelmet = 23 => "EN_GEAR_EQUIP_SUBTYPE_PLATE_HELMET",
        PlateShoe = 24 => "EN_GEAR_EQUIP_SUBTYPE_PLATE_SHOE",
        PlateGlove = 25 => "EN_GEAR_EQUIP_SUBTYPE_PLATE_GLOVE",
        Ring = 31 => "EN_GEAR_EQUIP_SUBTYPE_RING",
        Necklace = 32 => "EN_GEAR_EQUIP_SUBTYPE_NECKLACE",
        ConsumableItem = 41 => "EN_GEAR_EQUIP_SUBTYPE_CONSUMABLE_ITEM",
        HoldableItem = 42 => "EN_GEAR_EQUIP_SUBTYPE_HOLDABLE_ITEM",
        ThrowableItem = 51 => "EN_GEAR_EQUIP_SUBTYPE_THROWABLE_ITEM",
        BareHands = 61 => "EN_GEAR_EQUIP_SUBTYPE_BARE_HANDS",
        Sword = 62 => "EN_GEAR_EQUIP_SUBTYPE_SWORD",
        Shield = 63 => "EN_GEAR_EQUIP_SUBTYPE_SHIELD",
        LongSword = 64 => "EN_GEAR_EQUIP_SUBTYPE_LONG_SWORD",
        Dagger = 65 => "EN_GEAR_EQUIP_SUBTYPE_DAGGER",
        Mace = 66 => "EN_GEAR_EQUIP_SUBTYPE_MACE",
        HeavyCrossbow = 81 => "EN_GEAR_EQUIP_SUBTYPE_HEAVY_CROSSBOW",
        Staff = 101 => "EN_GEAR_EQUIP_SUBTYPE_STAFF",
        Relic = 102 => "EN_GEAR_EQUIP_SUBTYPE_RELIC",
        CrossbowbowAmmo = 151 => "EN_GEAR_EQUIP_SUBTYPE_CROSSBOWBOW_AMMO",
        CommonChest = 201 => "EN_GEAR_EQUIP_SUBTYPE_COMMON_CHEST",
        CommonTrousers = 202 => "EN_GEAR_EQUIP_SUBTYPE_COMMON_TROUSERS",
        CommonHelmet = 203 => "EN_GEAR_EQUIP_SUBTYPE_COMMON_HELMET",
        CommonShoe = 204 => "EN_GEAR_EQUIP_SUBTYPE_COMMON_SHOE",
        CommonGlove = 205 => "EN_GEAR_EQUIP_SUBTYPE_COMMON_GLOVE",
    }
}

config_enum! { /// `tgf.EnGearEquipmentType`
    EnGearEquipmentType {
        Base = 0 => "EN_GEAR_EQUIPMENT_TYPE_BASE",
        Weapon = 1 => "EN_GEAR_EQUIPMENT_TYPE_WEAPON",
        Armor = 2 => "EN_GEAR_EQUIPMENT_TYPE_ARMOR",
        WeaponAnim = 3 => "EN_GEAR_EQUIPMENT_TYPE_WEAPON_ANIM",
        Gearset = 4 => "EN_GEAR_EQUIPMENT_TYPE_GEARSET",
    }
}

config_enum! { /// `tgf.EnGearInlaidShapeType`
    EnGearInlaidShapeType {
        Invalid = 0 => "EN_GEAR_INLAID_SHAPE_TYPE_INVALID",
        Attack = 1 => "EN_GEAR_INLAID_SHAPE_TYPE_ATTACK",
        Defend = 2 => "EN_GEAR_INLAID_SHAPE_TYPE_DEFEND",
        Function = 3 => "EN_GEAR_INLAID_SHAPE_TYPE_FUNCTION",
        FirstLevel = 4 => "EN_GEAR_INLAID_SHAPE_TYPE_FIRST_LEVEL",
        GearSet = 5 => "EN_GEAR_INLAID_SHAPE_TYPE_GEAR_SET",
        V2Attack = 51 => "EN_GEAR_INLAID_SHAPE_TYPE_2_ATTACK",
        V2Defend = 52 => "EN_GEAR_INLAID_SHAPE_TYPE_2_DEFEND",
        V2Function = 53 => "EN_GEAR_INLAID_SHAPE_TYPE_2_FUNCTION",
        V2FirstLevel = 54 => "EN_GEAR_INLAID_SHAPE_TYPE_2_FIRST_LEVEL",
        V2GearSet = 55 => "EN_GEAR_INLAID_SHAPE_TYPE_2_GEAR_SET",
    }
}

config_enum! { /// `tgf.EnItemBringoutInGameLimitType`
    EnItemBringoutInGameLimitType {
        NoLimit = 0 => "EN_ITEM_BRING_OUT_IN_GAME_LIMIT_TYPE_NO_LIMIT",
        CanNotButAllowCrossLayer = 1 => "EN_ITEM_BRING_OUT_IN_GAME_LIMIT_TYPE_CAN_NOT_BUT_ALLOW_CROSS_LAYER",
        CanNotAll = 3 => "EN_ITEM_BRING_OUT_IN_GAME_LIMIT_TYPE_CAN_NOT_ALL",
    }
}

config_enum! { /// `tgf.EnItemChgMainType`
    EnItemChgMainType {
        Unknown = 0 => "EN_ICMT_UNKNOWN",
        Gm = 1 => "EN_ICMT_GM",
        Register = 2 => "EN_ICMT_REGISTER",
        Timeout = 3 => "EN_ICMT_TIMEOUT",
        Use = 11 => "EN_ICMT_USE",
        Sale = 12 => "EN_ICMT_SALE",
        Buy = 13 => "EN_ICMT_BUY",
        MailAttachment = 21 => "EN_ICMT_MAIL_ATTACHMENT",
        Quest = 31 => "EN_ICMT_QUEST",
        Lotery = 32 => "EN_ICMT_LOTERY",
        BattleFinish = 33 => "EN_ICMT_BATTLE_FINISH",
        MovePosition = 34 => "EN_ICMT_MOVE_POSITION",
        AddTestItem = 35 => "EN_ICMT_ADD_TEST_ITEM",
        MarketRefreshTab = 36 => "EN_ICMT_MARKET_REFRESH_TAB",
        RoleReset = 37 => "EN_ICMT_ROLE_RESET",
        Trade = 38 => "EN_ICMT_TRADE",
        BattleTicket = 39 => "EN_ICMT_BATTLE_TICKET",
        BattleBringOut = 40 => "EN_ICMT_BATTLE_BRING_OUT",
        BattleCost = 41 => "EN_ICMT_BATTLE_COST",
        DeleteRole = 42 => "EN_ICMT_DELETE_ROLE",
        InventoryPageAddCost = 43 => "EN_ICMT_INVENTORY_PAGE_ADD_COST",
        Gear = 44 => "EN_ICMT_GEAR",
        QuestSubmit = 45 => "EN_ICMT_QUEST_SUBMIT",
        BuyAutoUse = 46 => "EN_ICMT_BUY_AUTO_USE",
        AddItemAutoUse = 47 => "EN_ICMT_ADD_ITEM_AUTO_USE",
        Destoy = 48 => "EN_ICMT_DESTOY",
        RoleMoveSeason = 49 => "EN_ICMT_ROLE_MOVE_SEASON",
        DeleteInventoryPage = 50 => "EN_ICMT_DELETE_INVENTORY_PAGE",
        LevelCompetitionWin = 51 => "EN_ICMT_LEVEL_COMPETITION_WIN",
        LevelCompetitionLose = 52 => "EN_ICMT_LEVEL_COMPETITION_LOSE",
        LevelCompetitionTie = 53 => "EN_ICMT_LEVEL_COMPETITION_TIE",
        LevelCompetitionGiveUp = 54 => "EN_ICMT_LEVEL_COMPETITION_GIVE_UP",
        DeleteAutoChange = 55 => "EN_ICMT_DELETE_AUTO_CHANGE",
        BattleRecover = 56 => "EN_ICMT_BATTLE_RECOVER",
        RoleMithrilForceItemGuidReset = 57 => "EN_ICMT_ROLE_MITHRIL_FORCE_ITEM_GUID_RESET",
        GearInsurance = 58 => "EN_ICMT_GEAR_INSURANCE",
        GuildCreate = 81 => "EN_ICMT_GUILD_CREATE",
        GuildRename = 82 => "EN_ICMT_GUILD_RENAME",
        RankSettlement = 91 => "EN_ICMT_RANK_SETTLEMENT",
        MithrilForceReward = 92 => "EN_ICMT_MITHRIL_FORCE_REWARD",
        MithrilForceEquip = 93 => "EN_ICMT_MITHRIL_FORCE_EQUIP",
        Crafting = 101 => "EN_ICMT_CRAFTING",
        ActivityCenter = 102 => "EN_ICMT_ACTIVITY_CENTER",
        RoleReshapeFacePinching = 103 => "EN_ICMT_ROLE_RESHAPE_FACE_PINCHING",
        DlcPacketRecycle = 201 => "EN_ICMT_DLC_PACKET_RECYCLE",
        DlcPacketReceive = 202 => "EN_ICMT_DLC_PACKET_RECEIVE",
        MidasExternal = 998 => "EN_ICMT_MIDAS_EXTERNAL",
        Idip = 999 => "EN_ICMT_IDIP",
    }
}

config_enum! { /// `tgf.EnItemChgSubType`
    EnItemChgSubType {
        Default = 0 => "EN_ICST_DEFAULT",
        Rollback = 1 => "EN_ICST_ROLLBACK",
        IdipDelete = 2 => "EN_ICST_IDIP_DELETE",
        IdipModify = 3 => "EN_ICST_IDIP_MODIFY",
        Buy = 10 => "EN_ICST_BUY",
        Spend = 11 => "EN_ICST_SPEND",
        CraftingReceived = 51 => "EN_ICST_CRAFTING_RECEIVED",
        TradePurchaseCost = 101 => "EN_ICST_TRADE_PURCHASE_COST",
        TradeSalesCost = 102 => "EN_ICST_TRADE_SALES_COST",
        TradeReceived = 103 => "EN_ICST_TRADE_RECEIVED",
        TradeAutoRefund = 104 => "EN_ICST_TRADE_AUTO_REFUND",
        TradeAuctionCost = 105 => "EN_ICST_TRADE_AUCTION_COST",
        TradeBidCost = 106 => "EN_ICST_TRADE_BID_COST",
        TradeAuctionTax = 107 => "EN_ICST_TRADE_AUCTION_TAX",
        TradeLoginFixCost = 108 => "EN_ICST_TRADE_LOGIN_FIX_COST",
        GearRepair = 151 => "EN_ICST_GEAR_REPAIR",
        GearInlaid = 152 => "EN_ICST_GEAR_INLAID",
        GearDismantle = 153 => "EN_ICST_GEAR_DISMANTLE",
        GearDismantleCost = 154 => "EN_ICST_GEAR_DISMANTLE_COST",
        GearDismantleReward = 155 => "EN_ICST_GEAR_DISMANTLE_REWARD",
        GearGemPolishing = 156 => "EN_ICST_GEAR_GEM_POLISHING",
        GearSoulBound = 157 => "EN_ICST_GEAR_SOUL_BOUND",
        GearHeirloomUnlock = 158 => "EN_ICST_GEAR_HEIRLOOM_UNLOCK",
        GearHeirloomUnload = 159 => "EN_ICST_GEAR_HEIRLOOM_UNLOAD",
        GearHeirloomInst = 160 => "EN_ICST_GEAR_HEIRLOOM_INST",
        GearGemUnlock = 161 => "EN_ICST_GEAR_GEM_UNLOCK",
        ActivityCenterRandomRefresh = 201 => "EN_ICST_ACTIVITY_CENTER_RANDOM_REFRESH",
        ActivityCenterManualRefresh = 202 => "EN_ICST_ACTIVITY_CENTER_MANUAL_REFRESH",
        ActivityCenterManualReceive = 203 => "EN_ICST_ACTIVITY_CENTER_MANUAL_RECEIVE",
        LevelCompetitionWinAffix = 251 => "EN_ICST_LEVEL_COMPETITION_WIN_AFFIX",
        TakeOffMithrilItem = 301 => "EN_ICST_TAKE_OFF_MITHRIL_ITEM",
        ForceQueryMidas = 401 => "EN_ICST_FORCE_QUERY_MIDAS",
        ActivityTrial = 402 => "EN_ICST_ACTIVITY_TRIAL",
    }
}

config_enum! { /// `tgf.EnItemDynamicType`
    EnItemDynamicType {
        Player = 0 => "EN_ITEM_DYNAMIC_TYPE_PLAYER",
        RoleCareer = 1 => "EN_ITEM_DYNAMIC_TYPE_ROLE_CAREER",
    }
}

config_enum! { /// `tgf.EnItemMajorType`
    EnItemMajorType {
        None = 0 => "EN_ITEM_MAJOR_TYPE_NONE",
        Currency = 1 => "EN_ITEM_MAJOR_TYPE_CURRENCY",
        Crafts = 2 => "EN_ITEM_MAJOR_TYPE_CRAFTS",
        Consumables = 3 => "EN_ITEM_MAJOR_TYPE_CONSUMABLES",
        Equipment = 4 => "EN_ITEM_MAJOR_TYPE_EQUIPMENT",
        Gem = 5 => "EN_ITEM_MAJOR_TYPE_GEM",
    }
}

config_enum! { /// `tgf.EnItemMinorType`
    EnItemMinorType {
        None = 0 => "EN_ITEM_MINOR_TYPE_NONE",
        Currency = 1 => "EN_ITEM_MINOR_TYPE_CURRENCY",
        Crafts = 10 => "EN_ITEM_MINOR_TYPE_CRAFTS",
        ConsumablesThrowable = 20 => "EN_ITEM_MINOR_TYPE_CONSUMABLES_THROWABLE",
        ConsumablesPotion = 21 => "EN_ITEM_MINOR_TYPE_CONSUMABLES_POTION",
        EquipmentWeapon = 41 => "EN_ITEM_MINOR_TYPE_EQUIPMENT_WEAPON",
        EquipmentArmor = 42 => "EN_ITEM_MINOR_TYPE_EQUIPMENT_ARMOR",
        EquipmentAccessories = 43 => "EN_ITEM_MINOR_TYPE_EQUIPMENT_ACCESSORIES",
        GemAttack = 61 => "EN_ITEM_MINOR_TYPE_GEM_ATTACK",
        GemDefend = 62 => "EN_ITEM_MINOR_TYPE_GEM_DEFEND",
        GemFunction = 63 => "EN_ITEM_MINOR_TYPE_GEM_FUNCTION",
        GemFirstLevel = 64 => "EN_ITEM_MINOR_TYPE_GEM_FIRST_LEVEL",
        GemGearSet = 65 => "EN_ITEM_MINOR_TYPE_GEM_GEAR_SET",
        Gem2Attack = 71 => "EN_ITEM_MINOR_TYPE_GEM_2_ATTACK",
        Gem2Defend = 72 => "EN_ITEM_MINOR_TYPE_GEM_2_DEFEND",
        Gem2Function = 73 => "EN_ITEM_MINOR_TYPE_GEM_2_FUNCTION",
        Gem2FirstLevel = 74 => "EN_ITEM_MINOR_TYPE_GEM_2_FIRST_LEVEL",
        Gem2GearSet = 75 => "EN_ITEM_MINOR_TYPE_GEM_2_GEAR_SET",
    }
}

config_enum! { /// `tgf.EnItemRarity`
    EnItemRarity {
        None = 0 => "EN_ITEM_RARITY_NONE",
        Rotted = 1 => "EN_ITEM_RARITY_ROTTED",
        Normal = 2 => "EN_ITEM_RARITY_NORMAL",
        Uncommon = 3 => "EN_ITEM_RARITY_UNCOMMON",
        Enhanced = 4 => "EN_ITEM_RARITY_ENHANCED",
        Epic = 5 => "EN_ITEM_RARITY_EPIC",
        Legendary = 6 => "EN_ITEM_RARITY_LEGENDARY",
        Unique = 7 => "EN_ITEM_RARITY_UNIQUE",
        Exceptional = 8 => "EN_ITEM_RARITY_EXCEPTIONAL",
    }
}

config_enum! { /// `tgf.EnItemStorageType`
    EnItemStorageType {
        Invalid = 0 => "EN_ITEM_STORAGE_TYPE_INVALID",
        Inventory = 1 => "EN_ITEM_STORAGE_TYPE_INVENTORY",
        Backpack = 2 => "EN_ITEM_STORAGE_TYPE_BACKPACK",
        RoleWearing = 3 => "EN_ITEM_STORAGE_TYPE_ROLE_WEARING",
        SmallBackpack = 4 => "EN_ITEM_STORAGE_TYPE_SMALL_BACKPACK",
        InventoryWithoutXy = 101 => "EN_ITEM_STORAGE_TYPE_INVENTORY_WITHOUT_XY",
        BackpackWithoutXy = 102 => "EN_ITEM_STORAGE_TYPE_BACKPACK_WITHOUT_XY",
        Mail = 201 => "EN_ITEM_STORAGE_TYPE_MAIL",
    }
}

config_enum! { /// `tgf.EnItemType`
    EnItemType {
        None = 0 => "EN_ITEM_TYPE_NONE",
        Coin = 1 => "EN_ITEM_TYPE_COIN",
        Virtual = 2 => "EN_ITEM_TYPE_VIRTUAL",
        Item = 3 => "EN_ITEM_TYPE_ITEM",
        Role = 4 => "EN_ITEM_TYPE_ROLE",
        MailTmpl = 5 => "EN_ITEM_TYPE_MAIL_TMPL",
        RandomPool = 6 => "EN_ITEM_TYPE_RANDOM_POOL",
        MarketGoods = 7 => "EN_ITEM_TYPE_MARKET_GOODS",
        DirectBuyGoods = 8 => "EN_ITEM_TYPE_DIRECT_BUY_GOODS",
        SpecialBuyGoods = 9 => "EN_ITEM_TYPE_SPECIAL_BUY_GOODS",
        MonthcardGoods = 10 => "EN_ITEM_TYPE_MONTHCARD_GOODS",
        Monthcard = 11 => "EN_ITEM_TYPE_MONTHCARD",
        UnlockModule = 12 => "EN_ITEM_TYPE_UNLOCK_MODULE",
        SkinWidget = 13 => "EN_ITEM_TYPE_SKIN_WIDGET",
        PlayerIcon = 14 => "EN_ITEM_TYPE_PLAYER_ICON",
        ActivityExpItem = 1401 => "EN_ITEM_TYPE_ACTIVITY_EXP_ITEM",
        ActivityCoinItem = 1402 => "EN_ITEM_TYPE_ACTIVITY_COIN_ITEM",
        QuestId = 15 => "EN_ITEM_TYPE_QUEST_ID",
        SteamAcvmQuestId = 1501 => "EN_ITEM_TYPE_STEAM_ACVM_QUEST_ID",
        QuestPoolId = 16 => "EN_ITEM_TYPE_QUEST_POOL_ID",
        QuestGroupId = 1601 => "EN_ITEM_TYPE_QUEST_GROUP_ID",
        ItemNormalGuid = 17 => "EN_ITEM_TYPE_ITEM_NORMAL_GUID",
        FragmentsTypeId = 19 => "EN_ITEM_TYPE_FRAGMENTS_TYPE_ID",
        MinionTypeId = 20 => "EN_ITEM_TYPE_MINION_TYPE_ID",
        CraftsTypeId = 21 => "EN_ITEM_TYPE_CRAFTS_TYPE_ID",
        GearTypeId = 101 => "EN_ITEM_TYPE_GEAR_TYPE_ID",
        GearMithrilTypeId = 102 => "EN_ITEM_TYPE_GEAR_MITHRIL_TYPE_ID",
        GearHeirloomGuideTypeId = 111 => "EN_ITEM_TYPE_GEAR_HEIRLOOM_GUIDE_TYPE_ID",
        GearHeirloomInstTypeId = 112 => "EN_ITEM_TYPE_GEAR_HEIRLOOM_INST_TYPE_ID",
        GearGemGuideTypeId = 113 => "EN_ITEM_TYPE_GEAR_GEM_GUIDE_TYPE_ID",
        GearGemInstTypeId = 114 => "EN_ITEM_TYPE_GEAR_GEM_INST_TYPE_ID",
        SkinSuitId = 115 => "EN_ITEM_TYPE_SKIN_SUIT_ID",
        AcvmId = 116 => "EN_ITEM_TYPE_ACVM_ID",
        AcvmQuestId = 117 => "EN_ITEM_TYPE_ACVM_QUEST_ID",
        TiersQuestId = 118 => "EN_ITEM_TYPE_TIERS_QUEST_ID",
    }
}

config_enum! { /// `tgf.EnItemUeType`
    EnItemUeType {
        None = 0 => "EN_ITEM_UE_TYPE_NONE",
        EquipWeapon = 1 => "EN_ITEM_UE_TYPE_EQUIP_WEAPON",
        EquipArmor = 2 => "EN_ITEM_UE_TYPE_EQUIP_ARMOR",
        EquipAccessory = 3 => "EN_ITEM_UE_TYPE_EQUIP_ACCESSORY",
        ItemConsumable = 21 => "EN_ITEM_UE_TYPE_ITEM_CONSUMABLE",
        ItemThrowable = 22 => "EN_ITEM_UE_TYPE_ITEM_THROWABLE",
        ItemAmmo = 23 => "EN_ITEM_UE_TYPE_ITEM_AMMO",
        ItemTreasure = 41 => "EN_ITEM_UE_TYPE_ITEM_TREASURE",
        ItemLevel = 42 => "EN_ITEM_UE_TYPE_ITEM_LEVEL",
        ItemGem = 43 => "EN_ITEM_UE_TYPE_ITEM_GEM",
        ItemOther = 51 => "EN_ITEM_UE_TYPE_ITEM_OTHER",
    }
}

config_enum! { /// `tgf.EnItemWearingPart`
    EnItemWearingPart {
        Invalid = 0 => "EN_ITEM_WEARING_PART_TYPE_INVALID",
        Pair1rightweapon = 1 => "EN_ITEM_WEARING_PART_TYPE_PAIR1RIGHTWEAPON",
        Pair1leftweapon = 2 => "EN_ITEM_WEARING_PART_TYPE_PAIR1LEFTWEAPON",
        Pair2rightweapon = 3 => "EN_ITEM_WEARING_PART_TYPE_PAIR2RIGHTWEAPON",
        Pair2leftweapon = 4 => "EN_ITEM_WEARING_PART_TYPE_PAIR2LEFTWEAPON",
        Upperarmor = 11 => "EN_ITEM_WEARING_PART_TYPE_UPPERARMOR",
        Lowerarmor = 12 => "EN_ITEM_WEARING_PART_TYPE_LOWERARMOR",
        Helmet = 13 => "EN_ITEM_WEARING_PART_TYPE_HELMET",
        Shoe = 14 => "EN_ITEM_WEARING_PART_TYPE_SHOE",
        Glove = 15 => "EN_ITEM_WEARING_PART_TYPE_GLOVE",
        Necklace = 16 => "EN_ITEM_WEARING_PART_TYPE_NECKLACE",
        Leftring = 17 => "EN_ITEM_WEARING_PART_TYPE_LEFTRING",
        Rightring = 18 => "EN_ITEM_WEARING_PART_TYPE_RIGHTRING",
        Consumable1 = 41 => "EN_ITEM_WEARING_PART_TYPE_CONSUMABLE1",
        Consumable2 = 42 => "EN_ITEM_WEARING_PART_TYPE_CONSUMABLE2",
        Consumable3 = 43 => "EN_ITEM_WEARING_PART_TYPE_CONSUMABLE3",
        Throwable = 51 => "EN_ITEM_WEARING_PART_TYPE_THROWABLE",
        AdditionalWeapon1 = 101 => "EN_ITEM_WEARING_PART_TYPE_ADDITIONAL_WEAPON1",
        AdditionalWeapon2 = 102 => "EN_ITEM_WEARING_PART_TYPE_ADDITIONAL_WEAPON2",
        AdditionalWeapon3 = 103 => "EN_ITEM_WEARING_PART_TYPE_ADDITIONAL_WEAPON3",
        AdditionalWeapon4 = 104 => "EN_ITEM_WEARING_PART_TYPE_ADDITIONAL_WEAPON4",
        AdditionalWeapon5 = 105 => "EN_ITEM_WEARING_PART_TYPE_ADDITIONAL_WEAPON5",
        AdditionalWeapon6 = 106 => "EN_ITEM_WEARING_PART_TYPE_ADDITIONAL_WEAPON6",
        AdditionalWeapon7 = 107 => "EN_ITEM_WEARING_PART_TYPE_ADDITIONAL_WEAPON7",
        AdditionalWeapon8 = 108 => "EN_ITEM_WEARING_PART_TYPE_ADDITIONAL_WEAPON8",
        AdditionalWeapon9 = 109 => "EN_ITEM_WEARING_PART_TYPE_ADDITIONAL_WEAPON9",
    }
}

config_enum! { /// `tgf.config.EnLootUnluckyType`
    EnLootUnluckyType {
        Invalid = 0 => "EN_LOOT_UNLUCKY_TYPE_INVALID",
        Epic = 1 => "EN_LOOT_UNLUCKY_TYPE_EPIC",
        EpicWeapon = 2 => "EN_LOOT_UNLUCKY_TYPE_EPIC_WEAPON",
        Legendary = 3 => "EN_LOOT_UNLUCKY_TYPE_LEGENDARY",
    }
}

config_enum! { /// `tgf.EnLotteryAccessToKeysType`
    EnLotteryAccessToKeysType {
        Invalid = 0 => "EN_LOTTERY_ACCESS_TO_KEYS_TYPE_INVALID",
        GenericBattle = 1 => "EN_LOTTERY_ACCESS_TO_KEYS_TYPE_GENERIC_BATTLE",
    }
}

config_enum! { /// `tgf.EnLotteryItemType`
    EnLotteryItemType {
        Invalid = 0 => "EN_LOTTERY_ITEM_TYPE_INVALID",
        Gear = 1 => "EN_LOTTERY_ITEM_TYPE_GEAR",
    }
}

config_enum! { /// `tgf.EnMarketType`
    EnMarketType {
        Invalid = 0 => "EN_MARKET_TYPE_INVALID",
        Merchant = 1 => "EN_MARKET_TYPE_MERCHANT",
        Shoppingmall = 2 => "EN_MARKET_TYPE_SHOPPINGMALL",
        BuyDirectly = 3 => "EN_MARKET_TYPE_BUY_DIRECTLY",
        Special = 4 => "EN_MARKET_TYPE_SPECIAL",
        Dlc = 5 => "EN_MARKET_TYPE_DLC",
    }
}

config_enum! { /// `tgf.EnQuestDisplayType`
    EnQuestDisplayType {
        Default = 0 => "EN_QUEST_DISPLAY_TYPE_DEFAULT",
        OffAll = 1 => "EN_QUEST_DISPLAY_TYPE_OFF_ALL",
        OnUnlock = 2 => "EN_QUEST_DISPLAY_TYPE_ON_UNLOCK",
        OnReceived = 4 => "EN_QUEST_DISPLAY_TYPE_ON_RECEIVED",
        OnUnlockAndReceive = 6 => "EN_QUEST_DISPLAY_TYPE_ON_UNLOCK_AND_RECEIVE",
    }
}

config_enum! { /// `tgf.EnQuestProgressCountType`
    EnQuestProgressCountType {
        Undefine = 0 => "EN_QUEST_PROGRESS_COUNT_TYPE_UNDEFINE",
        Single = 1 => "EN_QUEST_PROGRESS_COUNT_TYPE_SINGLE",
        AddUp = 2 => "EN_QUEST_PROGRESS_COUNT_TYPE_ADD_UP",
    }
}

config_enum! { /// `tgf.EnQuestProgressType`
    EnQuestProgressType {
        BattleAutoComplete = 0 => "EN_QUEST_PROGRESS_TYPE_BATTLE_AUTO_COMPLETE",
        BattleVictory = 1 => "EN_QUEST_PROGRESS_TYPE_BATTLE_VICTORY",
        KillType = 2 => "EN_QUEST_PROGRESS_TYPE_KILL_TYPE",
        CountOfCoinsSpent = 3 => "EN_QUEST_PROGRESS_TYPE_COUNT_OF_COINS_SPENT",
        OnlineTime = 4 => "EN_QUEST_PROGRESS_TYPE_ONLINE_TIME",
        CumulativeLoginDays = 5 => "EN_QUEST_PROGRESS_TYPE_CUMULATIVE_LOGIN_DAYS",
        ConsecutiveLoginDays = 6 => "EN_QUEST_PROGRESS_TYPE_CONSECUTIVE_LOGIN_DAYS",
        BattleParticipate = 7 => "EN_QUEST_PROGRESS_TYPE_BATTLE_PARTICIPATE",
        PlayerLevelArrive = 8 => "EN_QUEST_PROGRESS_TYPE_PLAYER_LEVEL_ARRIVE",
        RoleTotalArrive = 9 => "EN_QUEST_PROGRESS_TYPE_ROLE_TOTAL_ARRIVE",
        ItemAcquire = 10 => "EN_QUEST_PROGRESS_TYPE_ITEM_ACQUIRE",
        ItemConsume = 11 => "EN_QUEST_PROGRESS_TYPE_ITEM_CONSUME",
        NewbieSpecialProgressFromClient = 12 => "EN_QUEST_PROGRESS_TYPE_NEWBIE_SPECIAL_PROGRESS_FROM_CLIENT",
        CompleteQuest = 13 => "EN_QUEST_PROGRESS_TYPE_COMPLETE_QUEST",
        KillActorStatId = 14 => "EN_QUEST_PROGRESS_TYPE_KILL_ACTOR_STAT_ID",
        InteractingStatId = 15 => "EN_QUEST_PROGRESS_TYPE_INTERACTING_STAT_ID",
        MovingArea = 16 => "EN_QUEST_PROGRESS_TYPE_MOVING_AREA",
        OpeningChest = 17 => "EN_QUEST_PROGRESS_TYPE_OPENING_CHEST",
        DestroyingObject = 18 => "EN_QUEST_PROGRESS_TYPE_DESTROYING_OBJECT",
        AliveTime = 19 => "EN_QUEST_PROGRESS_TYPE_ALIVE_TIME",
        ItemSubmit = 20 => "EN_QUEST_PROGRESS_TYPE_ITEM_SUBMIT",
        ConsecutiveDead = 21 => "EN_QUEST_PROGRESS_TYPE_CONSECUTIVE_DEAD",
        WorthLossInBattle = 22 => "EN_QUEST_PROGRESS_TYPE_WORTH_LOSS_IN_BATTLE",
        ResurrectTeam = 23 => "EN_QUEST_PROGRESS_TYPE_RESURRECT_TEAM",
        PlayerRoleLevelArriveByGuid = 24 => "EN_QUEST_PROGRESS_TYPE_PLAYER_ROLE_LEVEL_ARRIVE_BY_GUID",
        Loot = 25 => "EN_QUEST_PROGRESS_TYPE_LOOT",
        PlayerCausedTotalDamage = 26 => "EN_QUEST_PROGRESS_TYPE_PLAYER_CAUSED_TOTAL_DAMAGE",
        PlayerCausedTotalHeal = 27 => "EN_QUEST_PROGRESS_TYPE_PLAYER_CAUSED_TOTAL_HEAL",
        PlayerGotTotalDamage = 28 => "EN_QUEST_PROGRESS_TYPE_PLAYER_GOT_TOTAL_DAMAGE",
        PlayerTakeItemWorth = 29 => "EN_QUEST_PROGRESS_TYPE_PLAYER_TAKE_ITEM_WORTH",
        CumulativeLoginDaysAfterActivity = 30 => "EN_QUEST_PROGRESS_TYPE_CUMULATIVE_LOGIN_DAYS_AFTER_ACTIVITY",
        PlayerRoleLevelArriveByCareer = 31 => "EN_QUEST_PROGRESS_TYPE_PLAYER_ROLE_LEVEL_ARRIVE_BY_CAREER",
        PlayerAbility = 32 => "EN_QUEST_PROGRESS_TYPE_PLAYER_ABILITY",
        ItemAddSub = 33 => "EN_QUEST_PROGRESS_TYPE_ITEM_ADD_SUB",
        HeirloomUpgrade = 34 => "EN_QUEST_PROGRESS_TYPE_HEIRLOOM_UPGRADE",
        ItemUse = 35 => "EN_QUEST_PROGRESS_TYPE_ITEM_USE",
        PlayerAction = 36 => "EN_QUEST_PROGRESS_TYPE_PLAYER_ACTION",
        ItemAcquireWorth = 37 => "EN_QUEST_PROGRESS_TYPE_ITEM_ACQUIRE_WORTH",
        ItemSellCount = 38 => "EN_QUEST_PROGRESS_TYPE_ITEM_SELL_COUNT",
        ItemSellPrice = 39 => "EN_QUEST_PROGRESS_TYPE_ITEM_SELL_PRICE",
        TradeOpItem = 40 => "EN_QUEST_PROGRESS_TYPE_TRADE_OP_ITEM",
        TradeBuyItem = 41 => "EN_QUEST_PROGRESS_TYPE_TRADE_BUY_ITEM",
        TradeSalesItem = 42 => "EN_QUEST_PROGRESS_TYPE_TRADE_SALES_ITEM",
        GemUpgrade = 43 => "EN_QUEST_PROGRESS_TYPE_GEM_UPGRADE",
        ItemBuy = 44 => "EN_QUEST_PROGRESS_TYPE_ITEM_BUY",
        TradeBuyItemCost = 45 => "EN_QUEST_PROGRESS_TYPE_TRADE_BUY_ITEM_COST",
        TradeSalesItemCost = 46 => "EN_QUEST_PROGRESS_TYPE_TRADE_SALES_ITEM_COST",
        BattleRelationStat = 47 => "EN_QUEST_PROGRESS_TYPE_BATTLE_RELATION_STAT",
        TiersArrive = 48 => "EN_QUEST_PROGRESS_TYPE_TIERS_ARRIVE",
        BattlePortalTypeCount = 49 => "EN_QUEST_PROGRESS_TYPE_BATTLE_PORTAL_TYPE_COUNT",
        BattleItemAcquire = 50 => "EN_QUEST_PROGRESS_TYPE_BATTLE_ITEM_ACQUIRE",
        BattleSynthesizeCostItem = 51 => "EN_QUEST_PROGRESS_TYPE_BATTLE_SYNTHESIZE_COST_ITEM",
    }
}

config_enum! { /// `tgf.EnQuestProgressValueCompareType`
    EnQuestProgressValueCompareType {
        AutoComplete = 0 => "EN_QUEST_PROGRESS_VALUE_COMPARE_TYPE_AUTO_COMPLETE",
        GreaterOrEqual = 1 => "EN_QUEST_PROGRESS_VALUE_COMPARE_TYPE_GREATER_OR_EQUAL",
        LessOrEqual = 2 => "EN_QUEST_PROGRESS_VALUE_COMPARE_TYPE_LESS_OR_EQUAL",
        StrictlyEqual = 3 => "EN_QUEST_PROGRESS_VALUE_COMPARE_TYPE_STRICTLY_EQUAL",
    }
}

config_enum! { /// `tgf.config.EnQuestRewardGiveOutType`
    EnQuestRewardGiveOutType {
        Manual = 0 => "EN_QUEST_REWARD_GIVE_OUT_TYPE_MANUAL",
        AutoInventory = 1 => "EN_QUEST_REWARD_GIVE_OUT_TYPE_AUTO_INVENTORY",
        AutoMail = 2 => "EN_QUEST_REWARD_GIVE_OUT_TYPE_AUTO_MAIL",
        ManualMail = 3 => "EN_QUEST_REWARD_GIVE_OUT_TYPE_MANUAL_MAIL",
    }
}

config_enum! { /// `tgf.EnQuestTirggerType`
    EnQuestTirggerType {
        Invalid = 0 => "EN_QUEST_TRIGGER_TYPE_INVALID",
        PlayerLevelPromote = 1 => "EN_QUEST_TRIGGER_TYPE_PLAYER_LEVEL_PROMOTE",
        RoleAcquire = 2 => "EN_QUEST_TRIGGER_TYPE_ROLE_ACQUIRE",
        RankArrive = 3 => "EN_QUEST_TRIGGER_TYPE_RANK_ARRIVE",
        PlayerVipLevelPromote = 4 => "EN_QUEST_TRIGGER_TYPE_PLAYER_VIP_LEVEL_PROMOTE",
        QuestComplete = 5 => "EN_QUEST_TRIGGER_TYPE_QUEST_COMPLETE",
        QuestReceive = 6 => "EN_QUEST_TRIGGER_TYPE_QUEST_RECEIVE",
        BattleFinish = 7 => "EN_QUEST_TRIGGER_TYPE_BATTLE_FINISH",
        CostCoins = 8 => "EN_QUEST_TRIGGER_TYPE_COST_COINS",
        Login = 9 => "EN_QUEST_TRIGGER_TYPE_LOGIN",
        Online = 10 => "EN_QUEST_TRIGGER_TYPE_ONLINE",
        ItemAcquire = 11 => "EN_QUEST_TRIGGER_TYPE_ITEM_ACQUIRE",
        ItemConsume = 12 => "EN_QUEST_TRIGGER_TYPE_ITEM_CONSUME",
        TaskTimeTick = 13 => "EN_QUEST_TRIGGER_TYPE_TASK_TIME_TICK",
        ItemSubmit = 14 => "EN_QUEST_TRIGGER_TYPE_ITEM_SUBMIT",
        PlayerHisMaxLevelPromote = 15 => "EN_QUEST_TRIGGER_TYPE_PLAYER_HIS_MAX_LEVEL_PROMOTE",
        HeirloomUpgrade = 16 => "EN_QUEST_TRIGGER_TYPE_HEIRLOOM_UPGRADE",
        ItemUse = 17 => "EN_QUEST_TRIGGER_TYPE_ITEM_USE",
        SellItem = 18 => "EN_QUEST_TRIGGER_TYPE_SELL_ITEM",
        TradeBuyItem = 19 => "EN_QUEST_TRIGGER_TYPE_TRADE_BUY_ITEM",
        TradeSalesItem = 20 => "EN_QUEST_TRIGGER_TYPE_TRADE_SALES_ITEM",
        GemUpgrade = 21 => "EN_QUEST_TRIGGER_TYPE_GEM_UPGRADE",
        ItemBuy = 22 => "EN_QUEST_TRIGGER_TYPE_ITEM_BUY",
        LoginForAc = 23 => "EN_QUEST_TRIGGER_TYPE_LOGIN_FOR_AC",
        BattleItemAcquire = 24 => "EN_QUEST_TRIGGER_TYPE_BATTLE_ITEM_ACQUIRE",
    }
}

config_enum! { /// `tgf.EnQuestType`
    EnQuestType {
        Undefine = 0 => "EN_QUEST_TYPE_UNDEFINE",
        Daily = 1 => "EN_QUEST_TYPE_DAILY",
        Deadly = 2 => "EN_QUEST_TYPE_DEADLY",
        Newbie = 3 => "EN_QUEST_TYPE_NEWBIE",
        Adventure = 4 => "EN_QUEST_TYPE_ADVENTURE",
        Challenge = 5 => "EN_QUEST_TYPE_CHALLENGE",
        OperationalActivities = 6 => "EN_QUEST_TYPE_OPERATIONAL_ACTIVITIES",
        OperationalActivitiesSteam = 7 => "EN_QUEST_TYPE_OPERATIONAL_ACTIVITIES_STEAM",
        OperationalActivitiesPassport = 8 => "EN_QUEST_TYPE_OPERATIONAL_ACTIVITIES_PASSPORT",
    }
}

config_enum! { /// `tgf.EnRankLockType`
    EnRankLockType {
        None = 0 => "EN_RANK_LOCK_TYPE_NONE",
        Daily = 1 => "EN_RANK_LOCK_TYPE_DAILY",
        Custom = 2 => "EN_RANK_LOCK_TYPE_CUSTOM",
    }
}

config_enum! { /// `tgf.EnRankPeriodRewardType`
    EnRankPeriodRewardType {
        None = 0 => "EN_RANK_PREWARD_TYPE_NONE",
        ByRank = 1 => "EN_RANK_PREWARD_TYPE_BY_RANK",
        ByScore = 2 => "EN_RANK_PREWARD_TYPE_BY_SCORE",
    }
}

config_enum! { /// `tgf.EnRankSettlementType`
    EnRankSettlementType {
        None = 0 => "EN_RANK_SETTLEMENT_TYPE_NONE",
        Image = 1 => "EN_RANK_SETTLEMENT_TYPE_IMAGE",
        AbcBoardKeep = 2 => "EN_RANK_SETTLEMENT_TYPE_ABC_BOARD_KEEP",
        AbcBoardClear = 3 => "EN_RANK_SETTLEMENT_TYPE_ABC_BOARD_CLEAR",
        OriginalAndClear = 4 => "EN_RANK_SETTLEMENT_TYPE_ORIGINAL_AND_CLEAR",
    }
}

config_enum! { /// `tgf.EnRankSortType`
    EnRankSortType {
        Defult = 0 => "EN_RANK_SORT_TYPE_DEFULT",
        EqualScore = 1 => "EN_RANK_SORT_TYPE_EQUAL_SCORE",
    }
}

config_enum! { /// `tgf.EnRefreshPoolType`
    EnRefreshPoolType {
        Random = 0 => "EN_REFRESH_POOL_TYPE_RANDOM",
        RemoveDuplicate = 1 => "EN_REFRESH_POOL_TYPE_REMOVE_DUPLICATE",
    }
}

config_enum! { /// `tgf.EnRoleCareerType`
    EnRoleCareerType {
        Invalid = 0 => "EN_ROLE_CAREER_INVALID",
        Swordmaster = 1 => "EN_ROLE_CAREER_SWORDMASTER",
        Fighter = 2 => "EN_ROLE_CAREER_FIGHTER",
        Rogue = 3 => "EN_ROLE_CAREER_ROGUE",
        Priest = 4 => "EN_ROLE_CAREER_PRIEST",
        Wizard = 5 => "EN_ROLE_CAREER_WIZARD",
        Deathknight = 6 => "EN_ROLE_CAREER_DEATHKNIGHT",
        Lich = 7 => "EN_ROLE_CAREER_LICH",
        DruidPanther = 8 => "EN_ROLE_CAREER_DRUID_PANTHER",
        MithrilForceFighter = 101 => "EN_ROLE_CAREER_MITHRIL_FORCE_FIGHTER",
        MithrilForceRogue = 102 => "EN_ROLE_CAREER_MITHRIL_FORCE_ROGUE",
        MithrilForcePriest = 103 => "EN_ROLE_CAREER_MITHRIL_FORCE_PRIEST",
    }
}

config_enum! { /// `tgf.EnRoleGenderType`
    EnRoleGenderType {
        None = 0 => "EN_ROLE_GENDER_NONE",
        Male = 1 => "EN_ROLE_GENDER_MALE",
        Female = 2 => "EN_ROLE_GENDER_FEMALE",
        Undead = 3 => "EN_ROLE_GENDER_UNDEAD",
        Invalid = 127 => "EN_ROLE_GENDER_INVALID",
    }
}

config_enum! { /// `tgf.EnRoleRaceType`
    EnRoleRaceType {
        Invalid = 0 => "EN_ROLE_RACE_INVALID",
        Human = 1 => "EN_ROLE_RACE_HUMAN",
        NightElf = 2 => "EN_ROLE_RACE_NIGHT_ELF",
        Undead = 3 => "EN_ROLE_RACE_UNDEAD",
    }
}

config_enum! { /// `tgf.EnRoleSkillPositionType`
    EnRoleSkillPositionType {
        Invalid = 0 => "EN_ROLE_SKILL_POSITION_INVALID",
        Q = 1 => "EN_ROLE_SKILL_POSITION_Q",
        E = 2 => "EN_ROLE_SKILL_POSITION_E",
    }
}

config_enum! { /// `tgf.EnSkinCategoryType`
    EnSkinCategoryType {
        Normal = 0 => "EN_SKIN_CATEGORY_TYPE_NORMAL",
        Heirloom = 1 => "EN_SKIN_CATEGORY_TYPE_HEIRLOOM",
    }
}

config_enum! { /// `tgf.EnSkinSlotType`
    EnSkinSlotType {
        None = 0 => "EN_SKIN_SLOT_TYPE_NONE",
        ClothChest = 1 => "EN_SKIN_SLOT_TYPE_CLOTH_CHEST",
        ClothTrousers = 2 => "EN_SKIN_SLOT_TYPE_CLOTH_TROUSERS",
        ClothHelmet = 3 => "EN_SKIN_SLOT_TYPE_CLOTH_HELMET",
        ClothShoe = 4 => "EN_SKIN_SLOT_TYPE_CLOTH_SHOE",
        ClothGlove = 5 => "EN_SKIN_SLOT_TYPE_CLOTH_GLOVE",
        LeatherChest = 11 => "EN_SKIN_SLOT_TYPE_LEATHER_CHEST",
        LeatherTrousers = 12 => "EN_SKIN_SLOT_TYPE_LEATHER_TROUSERS",
        LeatherHelmet = 13 => "EN_SKIN_SLOT_TYPE_LEATHER_HELMET",
        LeatherShoe = 14 => "EN_SKIN_SLOT_TYPE_LEATHER_SHOE",
        LeatherGlove = 15 => "EN_SKIN_SLOT_TYPE_LEATHER_GLOVE",
        PlateChest = 21 => "EN_SKIN_SLOT_TYPE_PLATE_CHEST",
        PlateTrousers = 22 => "EN_SKIN_SLOT_TYPE_PLATE_TROUSERS",
        PlateHelmet = 23 => "EN_SKIN_SLOT_TYPE_PLATE_HELMET",
        PlateShoe = 24 => "EN_SKIN_SLOT_TYPE_PLATE_SHOE",
        PlateGlove = 25 => "EN_SKIN_SLOT_TYPE_PLATE_GLOVE",
        Cloak = 31 => "EN_SKIN_SLOT_TYPE_CLOAK",
        Headdress = 32 => "EN_SKIN_SLOT_TYPE_HEADDRESS",
        Torch = 41 => "EN_SKIN_SLOT_TYPE_TORCH",
        Potion = 51 => "EN_SKIN_SLOT_TYPE_POTION",
        Sword = 62 => "EN_SKIN_SLOT_TYPE_SWORD",
        Shield = 63 => "EN_SKIN_SLOT_TYPE_SHIELD",
        LongSword = 64 => "EN_SKIN_SLOT_TYPE_LONG_SWORD",
        Dagger = 65 => "EN_SKIN_SLOT_TYPE_DAGGER",
        Mace = 66 => "EN_SKIN_SLOT_TYPE_MACE",
        HeavyCrossBow = 81 => "EN_SKIN_SLOT_TYPE_HEAVY_CROSS_BOW",
        Staff = 101 => "EN_SKIN_SLOT_TYPE_STAFF",
        Relic = 102 => "EN_SKIN_SLOT_TYPE_RELIC",
        AttachingWaist = 71 => "EN_SKIN_SLOT_TYPE_ATTACHING_WAIST",
        AttachingBack = 72 => "EN_SKIN_SLOT_TYPE_ATTACHING_BACK",
        KillingEffect = 151 => "EN_SKIN_SLOT_TYPE_KILLING_EFFECT",
        Headframe = 201 => "EN_SKIN_SLOT_TYPE_HEADFRAME",
    }
}

config_enum! { /// `tgf.config.EnTradeItemRequireType`
    EnTradeItemRequireType {
        None = 0 => "EN_TRADE_ITEM_REQUIRE_TYPE_NONE",
        Corruption = 1 => "EN_TRADE_ITEM_REQUIRE_TYPE_CORRUPTION",
        GearSet = 2 => "EN_TRADE_ITEM_REQUIRE_TYPE_GEAR_SET",
    }
}

config_enum! { /// `tgf.config.EnTradeMarketType`
    EnTradeMarketType {
        Unknow = 0 => "EN_TRADE_MARKET_TYPE_UNKNOW",
        Auction = 1 => "EN_TRADE_MARKET_TYPE_AUCTION",
        Homogeneous = 2 => "EN_TRADE_MARKET_TYPE_HOMOGENEOUS",
    }
}

config_enum! { /// `tgf.config.EnTradeMatchMethod`
    EnTradeMatchMethod {
        MatchMethodRandom = 0 => "EN_TRADE_MATCH_METHOD_RANDOM",
        PriceMethodTime = 1 => "EN_TRADE_PRICE_METHOD_TIME",
        PriceMethodPrice = 2 => "EN_TRADE_PRICE_METHOD_PRICE",
    }
}

config_enum! { /// `tgf.config.EnTradePriceMethod`
    EnTradePriceMethod {
        Auto = 0 => "EN_TRADE_PRICE_METHOD_AUTO",
        Lock = 1 => "EN_TRADE_PRICE_METHOD_LOCK",
        LockByConfig = 2 => "EN_TRADE_PRICE_METHOD_LOCK_BY_CONFIG",
    }
}

config_enum! { /// `tgf.config.EnTradeTicketType`
    EnTradeTicketType {
        Unknow = 0 => "EN_TRADE_TICKET_TYPE_UNKNOW",
        Purchase = 1 => "EN_TRADE_TICKET_TYPE_PURCHASE",
        Fixed = 2 => "EN_TRADE_TICKET_TYPE_FIXED",
        Auction = 3 => "EN_TRADE_TICKET_TYPE_AUCTION",
    }
}

config_enum! { /// `tgf.config.EnUnlockModuleID`
    EnUnlockModuleID {
        Null = 0 => "EN_UNLOCK_MODULE_ID_NULL",
        Login = 1200000 => "EN_UNLOCK_MODULE_ID_LOGIN",
        Chat = 1200001 => "EN_UNLOCK_MODULE_ID_CHAT",
        ChatSilence = 1200002 => "EN_UNLOCK_MODULE_ID_CHAT_SILENCE",
        Friend = 1200003 => "EN_UNLOCK_MODULE_ID_FRIEND",
        Team = 1200004 => "EN_UNLOCK_MODULE_ID_TEAM",
        ChatVoice = 1200005 => "EN_UNLOCK_MODULE_ID_CHAT_VOICE",
        TradeHomogeneousSell = 1200012 => "EN_UNLOCK_MODULE_ID_TRADE_HOMOGENEOUS_SELL",
        TradeHomogeneousBuy = 1200013 => "EN_UNLOCK_MODULE_ID_TRADE_HOMOGENEOUS_BUY",
        TradeAuctionSell = 1200014 => "EN_UNLOCK_MODULE_ID_TRADE_AUCTION_SELL",
        TradeAuctionBuy = 1200015 => "EN_UNLOCK_MODULE_ID_TRADE_AUCTION_BUY",
        ChatOnlyEnglish = 1200016 => "EN_UNLOCK_MODULE_ID_CHAT_ONLY_ENGLISH",
        Rank = 1200017 => "EN_UNLOCK_MODULE_ID_RANK",
        FriendSearchByCreateIndex = 1200018 => "EN_UNLOCK_MODULE_ID_FRIEND_SEARCH_BY_CREATE_INDEX",
        PopupNotice = 1200019 => "EN_UNLOCK_MODULE_ID_POPUP_NOTICE",
        CastleNormal = 1200020 => "EN_UNLOCK_MODULE_ID_CASTLE_NORMAL",
        CastleHard = 1200021 => "EN_UNLOCK_MODULE_ID_CASTLE_HARD",
        DungeonNormal = 1200022 => "EN_UNLOCK_MODULE_ID_DUNGEON_NORMAL",
        DungeonHard = 1200023 => "EN_UNLOCK_MODULE_ID_DUNGEON_HARD",
        Pvp = 1200024 => "EN_UNLOCK_MODULE_ID_PVP",
        PrivateRoom = 1200025 => "EN_UNLOCK_MODULE_ID_PRIVATE_ROOM",
        Activity = 1200026 => "EN_UNLOCK_MODULE_ID_ACTIVITY",
        SmithRepair = 1200027 => "EN_UNLOCK_MODULE_ID_SMITH_REPAIR",
        SmithDismantle = 1200028 => "EN_UNLOCK_MODULE_ID_SMITH_DISMANTLE",
        Report = 1200029 => "EN_UNLOCK_MODULE_ID_REPORT",
        TradeEntry = 1200030 => "EN_UNLOCK_MODULE_ID_TRADE_ENTRY",
        MarketEntry = 1200031 => "EN_UNLOCK_MODULE_ID_MARKET_ENTRY",
        Pvp6v6 = 1200032 => "EN_UNLOCK_MODULE_ID_PVP_6V6",
        Mithril = 1200033 => "EN_UNLOCK_MODULE_ID_MITHRIL",
        Skin = 1200034 => "EN_UNLOCK_MODULE_ID_SKIN",
        Heirloom = 1200035 => "EN_UNLOCK_MODULE_ID_HEIRLOOM",
        CastleHell = 1200036 => "EN_UNLOCK_MODULE_ID_CASTLE_HELL",
        DungeonHell = 1200037 => "EN_UNLOCK_MODULE_ID_DUNGEON_HELL",
    }
}

config_enum! { /// `tgf.EnWeaponEquipType`
    EnWeaponEquipType {
        None = 0 => "EN_WEAPON_EQUIP_TYPE_NONE",
        Onlyright = 1 => "EN_WEAPON_EQUIP_TYPE_ONLYRIGHT",
        Onlyleft = 2 => "EN_WEAPON_EQUIP_TYPE_ONLYLEFT",
        Bothhand = 3 => "EN_WEAPON_EQUIP_TYPE_BOTHHAND",
        Consumable = 11 => "EN_WEAPON_EQUIP_TYPE_CONSUMABLE",
        Throwable = 12 => "EN_WEAPON_EQUIP_TYPE_THROWABLE",
    }
}

/// `tgf.config.BattleAffixEffectRuleEffect`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BattleAffixEffectRuleEffect {
    pub percentage: i32,
    pub offset: f32,
}

/// `tgf.DActivityDailyQuestRefreshCfg`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DActivityDailyQuestRefreshCfg {
    pub pool_id: i32,
    pub init_cnt: i32,
    pub add_cnt: i32,
    pub max_cnt: i32,
}

/// `tgf.DActivityMaxExp`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DActivityMaxExp {
    pub level_id: i32,
    pub max_exp: i32,
    pub cycle_exp: i32,
}

/// `tgf.DBattleAbillitySetOffset`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DBattleAbillitySetOffset {
    pub weight: I64,
}

/// `tgf.DBattleAffixAttributeOffset`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DBattleAffixAttributeOffset {
    pub min_value: f32,
    pub max_value: f32,
}

/// `tgf.DBattleAffixAttributeOffsetValue`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DBattleAffixAttributeOffsetValue {
    pub min_value: f32,
    pub max_value: f32,
    pub unit: f32,
}

/// `tgf.DBattleAffixAttributeValue`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DBattleAffixAttributeValue {
    pub affix_type: EnBattleAffixType,
    pub rarity_range: BTreeMap<String, DBattleAffixAttributeOffset>,
}

/// `tgf.DBattleAffixAttributeValueOffset`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DBattleAffixAttributeValueOffset {
    pub attribute_type: i32,
    pub unit: f32,
    pub affix_data: Vec<DBattleAffixAttributeValue>,
}

/// `tgf.DBattleAffixDisplayConfig`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DBattleAffixDisplayConfig {
    pub display_attack_right_att_gear_level: bool,
    pub display_defend_def_gear_level: bool,
}

/// `tgf.DBattleAffixFixedRule`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DBattleAffixFixedRule {
    pub affix_type: EnBattleAffixType,
    pub location: EnBattleAffixEffectiveLocation,
    pub attribute: Option<DBattleAttributeOffset>,
}

/// `tgf.DBattleAffixLegendaryOffset`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DBattleAffixLegendaryOffset {
    pub ability_set_id: i32,
    pub min_value: i32,
    pub max_value: i32,
    pub display_order: i32,
    pub role_display_order: i32,
}

/// `tgf.DBattleAffixLegendaryRule`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DBattleAffixLegendaryRule {
    pub affix_alias_id: i32,
    pub location: EnBattleAffixEffectiveLocation,
}

/// `tgf.DBattleAffixOffset`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DBattleAffixOffset {
    pub affix_value_id: i32,
    pub fixed_rule: Vec<DBattleAffixFixedRule>,
    pub random_rule: Vec<DBattleAffixRandomRule>,
    pub legendary_rule: Vec<DBattleAffixLegendaryRule>,
}

/// `tgf.DBattleAffixRandomRule`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DBattleAffixRandomRule {
    pub affix_type: EnBattleAffixType,
    pub affix_count_pool_id: i32,
    pub location: EnBattleAffixEffectiveLocation,
    pub pool_random: Vec<DBattleAffixRandomRulePool>,
    pub minor_affix_default_rule_text: String,
}

/// `tgf.DBattleAffixRandomRulePool`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DBattleAffixRandomRulePool {
    pub affix_pool_id: Vec<i32>,
    pub condition: Option<DBattleAffixRandomRulePoolCondition>,
}

/// `tgf.DBattleAffixRandomRulePoolCondition`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DBattleAffixRandomRulePoolCondition {
    pub corruption_type: EnAffixCorruptionGroup,
    pub level_rule_id: i32,
    pub monster_rule_id: i32,
}

/// `tgf.DBattleAttributeOffset`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DBattleAttributeOffset {
    pub ability_index_1: Option<DBattleAbillitySetOffset>,
    pub ability_index_2: Option<DBattleAbillitySetOffset>,
    pub ability_index_3: Option<DBattleAbillitySetOffset>,
    pub ability_index_4: Option<DBattleAbillitySetOffset>,
    pub ability_index_5: Option<DBattleAbillitySetOffset>,
    pub ability_index_6: Option<DBattleAbillitySetOffset>,
    pub first_level_strength: Option<DBattleAttributeValueOffset>,
    pub first_level_dexterity: Option<DBattleAttributeValueOffset>,
    pub first_level_stamina: Option<DBattleAttributeValueOffset>,
    pub first_level_intelligence: Option<DBattleAttributeValueOffset>,
    pub first_level_will: Option<DBattleAttributeValueOffset>,
    pub first_level_faith: Option<DBattleAttributeValueOffset>,
    pub all_first_level: Option<DBattleAttributeValueOffset>,
    pub attack_right_weapon_attack: Option<DBattleAttributeValueOffset>,
    pub attack_attack_bonus: Option<DBattleAttributeValueOffset>,
    pub attack_attack_boost_rate: Option<DBattleAttributeValueOffset>,
    pub attack_critical_rate: Option<DBattleAttributeValueOffset>,
    pub attack_crit_damage_rate: Option<DBattleAttributeValueOffset>,
    pub attack_element_boost_rate_none: Option<DBattleAttributeValueOffset>,
    pub attack_element_boost_rate_all: Option<DBattleAttributeValueOffset>,
    pub attack_element_boost_rate_fire: Option<DBattleAttributeValueOffset>,
    pub attack_element_boost_rate_ice: Option<DBattleAttributeValueOffset>,
    pub attack_element_boost_rate_thunder: Option<DBattleAttributeValueOffset>,
    pub attack_element_boost_rate_light: Option<DBattleAttributeValueOffset>,
    pub attack_element_boost_rate_dark: Option<DBattleAttributeValueOffset>,
    pub attack_def_ignore_base: Option<DBattleAttributeValueOffset>,
    pub attack_physical_penetration: Option<DBattleAttributeValueOffset>,
    pub attack_fire_penetration: Option<DBattleAttributeValueOffset>,
    pub attack_cold_penetration: Option<DBattleAttributeValueOffset>,
    pub attack_lightning_penetration: Option<DBattleAttributeValueOffset>,
    pub attack_divine_penetration: Option<DBattleAttributeValueOffset>,
    pub attack_shadow_penetration: Option<DBattleAttributeValueOffset>,
    pub attack_right_att_gear_level: Option<DBattleAttributeValueOffset>,
    pub attack_extra_att_gear_level: Option<DBattleAttributeValueOffset>,
    pub attack_skill_boost_rate: Option<DBattleAttributeValueOffset>,
    pub defend_max_health_base: Option<DBattleAttributeValueOffset>,
    pub defend_max_health_percent: Option<DBattleAttributeValueOffset>,
    pub defend_max_health_extra: Option<DBattleAttributeValueOffset>,
    pub defend_def_gear_level: Option<DBattleAttributeValueOffset>,
    pub defend_extra_def_gear_level: Option<DBattleAttributeValueOffset>,
    pub defend_guarded_damage_reduce_rate: Option<DBattleAttributeValueOffset>,
    pub defend_element_resistance_rate_none: Option<DBattleAttributeValueOffset>,
    pub defend_element_resistance_rate_all: Option<DBattleAttributeValueOffset>,
    pub defend_element_resistance_rate_fire: Option<DBattleAttributeValueOffset>,
    pub defend_element_resistance_rate_ice: Option<DBattleAttributeValueOffset>,
    pub defend_element_resistance_rate_thunder: Option<DBattleAttributeValueOffset>,
    pub defend_element_resistance_rate_light: Option<DBattleAttributeValueOffset>,
    pub defend_element_resistance_rate_dark: Option<DBattleAttributeValueOffset>,
    pub defend_extra_guarded_damage_reduce_rate: Option<DBattleAttributeValueOffset>,
    pub speed_move_speed_base: Option<DBattleAttributeValueOffset>,
    pub speed_level_interact_speed_rate: Option<DBattleAttributeValueOffset>,
    pub recover_life_leach_rate: Option<DBattleAttributeValueOffset>,
    pub recover_heal_boost_rate: Option<DBattleAttributeValueOffset>,
    pub recover_heal_on_hit_base: Option<DBattleAttributeValueOffset>,
    pub recover_heal_on_kill_base: Option<DBattleAttributeValueOffset>,
    pub recover_heal_over_time_base: Option<DBattleAttributeValueOffset>,
    pub recover_shield_boost_rate: Option<DBattleAttributeValueOffset>,
    pub recover_heal_wound_rate: Option<DBattleAttributeValueOffset>,
    pub recover_skill_cooldown_scale_rate: Option<DBattleAttributeValueOffset>,
    pub recover_gain_gold_out_of_portal: Option<DBattleAttributeValueOffset>,
    pub recover_exp_boost_rate_out_of_portal: Option<DBattleAttributeValueOffset>,
    pub magic_max_magic_base: Option<DBattleAttributeValueOffset>,
    pub magic_magic_over_time: Option<DBattleAttributeValueOffset>,
    pub magic_magic_on_hit: Option<DBattleAttributeValueOffset>,
    pub magic_magic_on_kill: Option<DBattleAttributeValueOffset>,
    pub magic_magic_on_hurt: Option<DBattleAttributeValueOffset>,
    pub magic_magic_generate_rate: Option<DBattleAttributeValueOffset>,
    pub soul_energy_max_soul_energy_base: Option<DBattleAttributeValueOffset>,
    pub soul_energy_soul_energy_on_hit: Option<DBattleAttributeValueOffset>,
    pub soul_energy_soul_energy_on_kill: Option<DBattleAttributeValueOffset>,
    pub soul_energy_soul_energy_on_hurt: Option<DBattleAttributeValueOffset>,
    pub soul_energy_soul_energy_generate_rate: Option<DBattleAttributeValueOffset>,
    pub natural_energy_max_natural_energy_base: Option<DBattleAttributeValueOffset>,
    pub natural_energy_natural_energy_over_time: Option<DBattleAttributeValueOffset>,
    pub natural_energy_natural_energy_on_hit: Option<DBattleAttributeValueOffset>,
    pub natural_energy_natural_energy_on_kill: Option<DBattleAttributeValueOffset>,
    pub natural_energy_natural_energy_on_hurt: Option<DBattleAttributeValueOffset>,
    pub natural_energy_natural_energy_generate_rate: Option<DBattleAttributeValueOffset>,
    pub buff_arg_killing_max_layer_max_health_percent: Option<DBattleAttributeValueOffset>,
    pub buff_arg_killing_max_layer_attack_boost_rate: Option<DBattleAttributeValueOffset>,
    pub buff_arg_killing_max_layer_critical_rate: Option<DBattleAttributeValueOffset>,
    pub stat_effect_move_speed_with_hit_running: Option<DBattleAttributeValueOffset>,
    pub stat_effect_heal_over_time_with_hit_running: Option<DBattleAttributeValueOffset>,
    pub stat_effect_critical_rate_with_hit_running: Option<DBattleAttributeValueOffset>,
    pub stat_effect_life_leach_rate_with_hit_running: Option<DBattleAttributeValueOffset>,
    pub stat_effect_attack_boost_rate_with_full_health: Option<DBattleAttributeValueOffset>,
    pub stat_effect_critical_rate_with_full_health: Option<DBattleAttributeValueOffset>,
    pub trigger_effect_heal_on_open_chest: Option<DBattleAttributeValueOffset>,
    pub trigger_effect_magic_on_open_chest: Option<DBattleAttributeValueOffset>,
    pub trigger_effect_gain_gold_on_open_chest: Option<DBattleAttributeValueOffset>,
    pub trigger_effect_gain_gold_on_kill: Option<DBattleAttributeValueOffset>,
    pub trigger_effect_heal_on_defend_success: Option<DBattleAttributeValueOffset>,
    pub trigger_effect_damage_on_defend_success: Option<DBattleAttributeValueOffset>,
    pub trigger_effect_additional_fixed_damage_on_causing_hit: Option<DBattleAttributeValueOffset>,
    pub trigger_effect_add_shield_on_cast_skill: Option<DBattleAttributeValueOffset>,
    pub trigger_effect_additional_element_none_damage_on_causing_hit: Option<DBattleAttributeValueOffset>,
    pub trigger_effect_additional_element_fire_damage_on_causing_hit: Option<DBattleAttributeValueOffset>,
    pub trigger_effect_additional_element_ice_damage_on_causing_hit: Option<DBattleAttributeValueOffset>,
    pub trigger_effect_additional_element_thunder_damage_on_causing_hit: Option<DBattleAttributeValueOffset>,
    pub trigger_effect_additional_element_light_damage_on_causing_hit: Option<DBattleAttributeValueOffset>,
    pub trigger_effect_additional_element_dark_damage_on_causing_hit: Option<DBattleAttributeValueOffset>,
    pub gem_slot_begin: Option<DBattleGemSlotOffset>,
    pub gem_slot_attack: Option<DBattleGemSlotOffset>,
    pub gem_slot_defend: Option<DBattleGemSlotOffset>,
    pub gem_slot_function: Option<DBattleGemSlotOffset>,
    pub gem_slot_first_level: Option<DBattleGemSlotOffset>,
    pub gem_slot_gear_set: Option<DBattleGemSlotOffset>,
    pub gem_slot_2_attack: Option<DBattleGemSlotOffset>,
    pub gem_slot_2_defend: Option<DBattleGemSlotOffset>,
    pub gem_slot_2_function: Option<DBattleGemSlotOffset>,
    pub gem_slot_2_first_level: Option<DBattleGemSlotOffset>,
    pub gem_slot_2_gear_set: Option<DBattleGemSlotOffset>,
    pub gear_set_fleet_foot_work: Option<DBattleGearSetOffset>,
    pub gear_set_kleptomancy: Option<DBattleGearSetOffset>,
    pub gear_set_undying_grasp: Option<DBattleGearSetOffset>,
    pub gear_set_thunder_law: Option<DBattleGearSetOffset>,
    pub gear_set_bone_plating: Option<DBattleGearSetOffset>,
    pub gear_set_avoid_death: Option<DBattleGearSetOffset>,
    pub gear_set_scorching_touch: Option<DBattleGearSetOffset>,
    pub gear_set_summoning_aery: Option<DBattleGearSetOffset>,
    pub gear_set_coin_master: Option<DBattleGearSetOffset>,
    pub ability_set: Vec<DBattleAbillitySetOffset>,
}

/// `tgf.DBattleAttributeValueOffset`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DBattleAttributeValueOffset {
    pub weight: I64,
    pub offset: Option<DBattleAffixAttributeOffsetValue>,
    pub fixed_value: f32,
}

/// `tgf.DBattleGearServerAffixScore`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DBattleGearServerAffixScore {
    pub attack_score: i32,
    pub defend_score: i32,
}

/// `tgf.DBattleGearSetOffset`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DBattleGearSetOffset {
    pub weight: I64,
    pub fixed_value: i32,
}

/// `tgf.DBattleGemSlotOffset`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DBattleGemSlotOffset {
    pub weight: I64,
    pub fixed_count: i32,
}

/// `tgf.DCoin`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DCoin {
    pub coin_type_id: i32,
    pub count: I64,
    pub limit_option: Option<DLimitOption>,
}

/// `tgf.DCompetitionCost`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DCompetitionCost {
    pub item_id: i32,
    pub count: i32,
}

/// `tgf.DCompetitionSeasonDetail`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DCompetitionSeasonDetail {
    pub match_making_pool_id: i32,
    pub condition_id: i32,
    pub competition_rule_id: i32,
    pub competition_ranking_id: i32,
    pub judge_rule_id: i32,
    pub competition_reward_id: i32,
    pub competition_costs: Vec<DCompetitionCost>,
}

/// `tgf.DCompetitionSeasonTime`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DCompetitionSeasonTime {
    pub show_time: String,
    pub begin_time: String,
    pub end_time: String,
    pub freeze_begin_time: i32,
    pub freeze_end_time: i32,
}

/// `tgf.DConditionBasicLimit`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DConditionBasicLimit {
    pub show_time: String,
    pub valid_time: Vec<DConditionRuleRangeDatetime>,
    pub rule: Vec<DConditionRule>,
}

/// `tgf.DConditionRulInt`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DConditionRulInt {
    pub values: i32,
}

/// `tgf.DConditionRule`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DConditionRule {
    pub login_channel: Option<DConditionRuleMultipleUInt>,
    pub system_platform: Option<DConditionRuleMultipleUInt>,
    pub player_level: Option<DConditionRuleRangeInt>,
    pub team_member_count: Option<DConditionRuleRangeInt>,
    pub register_time: Option<DConditionRuleRangeDatetime>,
    pub client_version: Option<DConditionRuleRangeVersion>,
    pub role_current_max_level: Option<DConditionRuleRangeInt>,
    pub role_history_max_level: Option<DConditionRuleRangeInt>,
    pub aduit_limit: Option<DConditionRulInt>,
}

/// `tgf.DConditionRuleMultipleUInt`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DConditionRuleMultipleUInt {
    pub values: Vec<u32>,
}

/// `tgf.DConditionRuleRangeDatetime`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DConditionRuleRangeDatetime {
    pub begin_time: String,
    pub end_time: String,
}

/// `tgf.DConditionRuleRangeInt`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DConditionRuleRangeInt {
    pub left: i32,
    pub right: i32,
}

/// `tgf.DConditionRuleRangeTime`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DConditionRuleRangeTime {
    pub begin_time: String,
    pub end_time: String,
}

/// `tgf.DConditionRuleRangeVersion`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DConditionRuleRangeVersion {
    pub begin: String,
    pub end: String,
}

/// `tgf.DConfigCostItem`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DConfigCostItem {
    pub index: i32,
    pub items: Vec<DItemOffset>,
}

/// `tgf.config.DExcelCompetitionLevelGearRarityRequire`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DExcelCompetitionLevelGearRarityRequire {
    pub rarity: Vec<EnItemRarity>,
}

/// `tgf.config.DExcelLootChooseGroupRateCondition`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DExcelLootChooseGroupRateCondition {
    pub random_id: I64,
    pub max_loot_cnt: i32,
    pub unlucky_loot_cnt: i32,
    pub priority: i32,
}

/// `tgf.DExcelUeXYZ`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DExcelUeXYZ {
    #[serde(rename = "X")]
    pub x: f32,
    #[serde(rename = "Y")]
    pub y: f32,
    #[serde(rename = "Z")]
    pub z: f32,
}

/// `tgf.DFunctionUnlockCondition`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DFunctionUnlockCondition {
    pub role_level: i32,
    pub unlock_by_other_system: i32,
    pub season_role_level: Option<DFunctionUnlockConditionPairInt>,
}

/// `tgf.DFunctionUnlockConditionPairInt`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DFunctionUnlockConditionPairInt {
    pub k: i32,
    pub v: i32,
}

/// `tgf.config.DGearDurabilityLoss`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DGearDurabilityLoss {
    pub min: i32,
    pub max: i32,
}

/// `tgf.DIntKV`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DIntKV {
    pub k: i32,
    pub v: i32,
}

/// `tgf.DIntTriple`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DIntTriple {
    pub a: i32,
    pub b: i32,
    pub c: i32,
}

/// `tgf.DItemOffset`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DItemOffset {
    pub type_id: i32,
    pub count: i32,
    pub expire_offset: I64,
}

/// `tgf.DItemPositionCfg`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DItemPositionCfg {
    pub accumulation_limit: i32,
    pub row_size: i32,
    pub column_size: i32,
}

/// `tgf.DItemUseAction`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DItemUseAction {
    pub is_auto_use: bool,
    pub add_energy: i32,
    pub random_pool: i32,
    pub recipe_id: i32,
    pub add_inventory_page: i32,
    pub soul_bound: i32,
    pub add_facade: i32,
}

/// `tgf.DLimitOption`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DLimitOption {
    pub season: Option<DLimitOptionSeason>,
}

/// `tgf.DLimitOptionSeason`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DLimitOptionSeason {
    pub season_id: i32,
}

/// `tgf.DLotteryKeyAccess`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DLotteryKeyAccess {
    pub r#type: EnLotteryAccessToKeysType,
    pub limit: i32,
    pub period: Option<DLotteryKeyLimitResetInfo>,
    pub count: i32,
}

/// `tgf.DLotteryKeyLimitResetInfo`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DLotteryKeyLimitResetInfo {
    pub period_days: i32,
    pub start_day_reset_timepoint: String,
}

/// `tgf.DMailTemplate`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DMailTemplate {
    pub major_type: i32,
    pub minor_type: i32,
    pub is_admin_sender: bool,
    pub title: String,
    pub content: String,
    pub expired_duration_s: i32,
    pub show_time: String,
    pub start_time: String,
    pub expired_time: String,
    pub is_padding_expired_time: bool,
    pub resolve_expired_time: i32,
    pub condition_id: i32,
    pub attachments: Vec<DItemOffset>,
    pub keep_after_take_attachments: bool,
}

/// `tgf.config.DMapDynamicDSOption`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DMapDynamicDSOption {
    pub count: i32,
    pub args: Vec<i32>,
}

/// `tgf.DModuleCircularData`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DModuleCircularData {
    pub module_id: i32,
    pub duration: I64,
}

/// `tgf.config.DQuestAvailablePeriodType`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DQuestAvailablePeriodType {
    pub r#type: i32,
    pub specific_period: Option<DQuestSpecificAvailableTimePeriod>,
    pub timedesc: I64,
    pub specific_season_before: Option<DQuestSpecificAvailableSeasonPeriod>,
    pub package_version: String,
    pub resource_version: String,
}

/// `tgf.config.DQuestConditionProgress`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DQuestConditionProgress {
    pub type_id: EnQuestProgressType,
    pub condition_ids: Vec<I64>,
    pub value: I64,
    pub count_type: EnQuestProgressCountType,
    pub entity_args: Vec<i32>,
    pub ignore_entity_args: Vec<i32>,
    pub season_id: i32,
}

/// `tgf.config.DQuestDynamicReward`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DQuestDynamicReward {
    pub r#type: EnItemDynamicType,
    pub details: Vec<DQuestDynamicRewardItem>,
}

/// `tgf.config.DQuestDynamicRewardItem`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DQuestDynamicRewardItem {
    pub key: i32,
    pub items: Vec<DItemOffset>,
}

/// `tgf.config.DQuestProgressResetInfo`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DQuestProgressResetInfo {
    pub period_days: i32,
    pub start_day_reset_timepoint: String,
}

/// `tgf.config.DQuestReward`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DQuestReward {
    pub items: Vec<DItemOffset>,
    pub give_out_type: EnQuestRewardGiveOutType,
    pub mail_template_id: i32,
    pub dynamic: Option<DQuestDynamicReward>,
}

/// `tgf.config.DQuestSpecificAvailableSeasonPeriod`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DQuestSpecificAvailableSeasonPeriod {
    pub competition_level_id: i32,
    pub season_id: i32,
}

/// `tgf.config.DQuestSpecificAvailableTimePeriod`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DQuestSpecificAvailableTimePeriod {
    pub start: String,
    pub end: String,
}

/// `tgf.DQuestUnlockConditionItem`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DQuestUnlockConditionItem {
    pub player_level: I64,
    pub role_total: I64,
    pub rank_arrive: I64,
    pub vip_level: I64,
    pub previous_quest: I64,
    pub start_timepoint: I64,
    pub has_specify_role: I64,
    pub add_by_out_business_system: I64,
    pub season_id: i32,
}

/// `tgf.DRankPeriodRewardPoolContent`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DRankPeriodRewardPoolContent {
    pub reward: Vec<DItemOffset>,
    pub mail_template_id: i32,
    pub rank_name: String,
    pub rank_description: String,
    pub elo_k: i32,
    pub elo_rating_point: i32,
    pub score_change_type: ENRankScoreSettleType,
    pub score_change_param: i32,
}

/// `tgf.DRankPeriodRewardPoolIndex`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DRankPeriodRewardPoolIndex {
    pub rank_reward_pool_id: i32,
    pub reward_type: EnRankPeriodRewardType,
    pub interval_days: i32,
    pub lock_first_days: i32,
    pub lock_last_days: i32,
    pub save_history: bool,
    pub local_mode_season_settlement_set_score: i32,
}

/// `tgf.DRankRuleBindLevel`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DRankRuleBindLevel {
    pub level_type: i32,
    pub level_id: i32,
}

/// `tgf.DRankRuleContent`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DRankRuleContent {
    pub sub_rank_type: u32,
    pub sub_rank_instance_id: u32,
    pub shared_zone_id: u32,
    pub name: String,
    pub description: String,
    pub valid_time: Option<DConditionRuleRangeDatetime>,
    pub bind_level: Option<DRankRuleBindLevel>,
    pub settlement_type: EnRankSettlementType,
    pub settlement_time_offset: String,
    pub lock_type: EnRankLockType,
    pub lock_time: Option<DConditionRuleRangeTime>,
    pub activity_id: i32,
    pub sort_type: EnRankSortType,
    pub daily_settlement: Option<DRankPeriodRewardPoolIndex>,
    pub custom_settlement: Option<DRankPeriodRewardPoolIndex>,
}

/// `tgf.DRoleMetaIndex`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DRoleMetaIndex {
    pub race: EnRoleRaceType,
    pub career: EnRoleCareerType,
    pub gender: EnRoleGenderType,
    pub force: EnForceType,
}

/// `tgf.DRoleSkillKeyPosition`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DRoleSkillKeyPosition {
    pub key_position: EnRoleSkillPositionType,
    pub skill_id: i32,
}

/// `tgf.DTrialDisplayerCond`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DTrialDisplayerCond {
    pub win_count: i32,
    pub failed_count: i32,
}

/// `tgf.DTrialLevelCond`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DTrialLevelCond {
    pub win_range: Option<DTrialLevelRange>,
    pub failed_range: Option<DTrialLevelRange>,
}

/// `tgf.DTrialLevelRange`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DTrialLevelRange {
    pub min_count: i32,
    pub max_count: i32,
}

/// `tgf.config.DUeSourceBrushData`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DUeSourceBrushData {
    pub resource_name: String,
    pub image_size: Option<DUeSourceBrushSizeData>,
}

/// `tgf.config.DUeSourceBrushSizeData`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DUeSourceBrushSizeData {
    pub x: f64,
    pub y: f64,
}

/// `tgf.config.EquipmentGearConfig`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EquipmentGearConfig {
    pub is_gear: bool,
    pub available_equip_slot: Vec<EnItemWearingPart>,
    pub career_restrict: Vec<EnRoleCareerType>,
}

/// `tgf.config.EquipmentSubTypeCareerConfig`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EquipmentSubTypeCareerConfig {
    pub equip_subtype: EnGearEquipSubType,
    pub career_restrict: Vec<EnRoleCareerType>,
}

/// `tgf.config.EquipmentWeaponConfig`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EquipmentWeaponConfig {
    pub is_weapon: bool,
    pub weapon_equip_type: EnWeaponEquipType,
    pub can_equip_double: bool,
    pub career_restrict: Vec<EnRoleCareerType>,
    pub secondary_weapon_types: Vec<EnGearEquipSubType>,
}

/// `tgf.config.ExcelActivityCenter`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelActivityCenter {
    pub id: i32,
    pub on: bool,
    pub priority: i32,
    pub start_time: String,
    pub end_time: String,
    pub expired_time: String,
    pub daily_limit: i32,
    pub entitys: Vec<DIntKV>,
    pub name: String,
    pub red_dot_refresh: i32,
    pub common_condition: Option<DConditionBasicLimit>,
    pub expired_type: i32,
    pub r#type: EnActivityType,
    pub manual_refresh_costs: Vec<DIntKV>,
    pub rule_desc: String,
    pub main_pic: String,
    pub minor_pics: Vec<String>,
    pub description: String,
    pub popup_level: i32,
    pub reward_mail_template_id: i32,
    pub max_progress_quest_id: i32,
    pub client_custom_text: String,
    pub duration_days: i32,
    pub client_hidden: bool,
    pub exp_item_type: i32,
    pub exp_cost: Option<DIntKV>,
    pub daily_quest_cfg: Option<DActivityDailyQuestRefreshCfg>,
    pub activity_rule: i32,
    pub bind_levels: Vec<i32>,
    pub grade_max_exp: Option<DActivityMaxExp>,
    pub custom_period_days: i32,
    pub period: i32,
    pub seaon_id: i32,
}

/// `tgf.config.ExcelActivityCenterPassprotsReards`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelActivityCenterPassprotsReards {
    pub id: i32,
    pub activity_id: i32,
    pub level: i32,
    pub exp_begin: i32,
    pub exp_end: i32,
    pub basic_rewards_id: i32,
    pub basic_rewards: Vec<DItemOffset>,
    pub high_rewards_id: i32,
    pub high_rewards: Vec<DItemOffset>,
}

/// `tgf.config.ExcelActivityCenterRandomRewards`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelActivityCenterRandomRewards {
    pub id: i32,
    pub activity_id: i32,
    pub career_type_id: i32,
    pub selected_max: i32,
    pub item_cnt_pool: Vec<DIntKV>,
    pub item_randoom_pool_id: i32,
}

/// `tgf.config.ExcelActivityCenterRandomRewardsPool`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelActivityCenterRandomRewardsPool {
    pub id: i32,
    pub r#type: i32,
    pub random_item: Vec<DIntTriple>,
}

/// `tgf.config.ExcelAffixAbilitySetId`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelAffixAbilitySetId {
    pub affix_index_id: i32,
    pub ability_set_id: i32,
}

/// `tgf.config.ExcelAffixCountRandomPool`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelAffixCountRandomPool {
    pub affix_count_pool_id: i32,
    pub count: i32,
    pub random_weight: I64,
}

/// `tgf.config.ExcelAffixGearSetMapRule`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelAffixGearSetMapRule {
    pub gear_set_type: i32,
    pub equip_id: i32,
}

/// `tgf.config.ExcelAffixGearSetRule`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelAffixGearSetRule {
    pub gear_set_type: i32,
    pub level: i32,
    pub text: String,
    pub unlock_level: i32,
}

/// `tgf.config.ExcelAffixLegendaryRule`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelAffixLegendaryRule {
    pub affix_alias_id: i32,
    pub offset: Option<DBattleAffixLegendaryOffset>,
}

/// `tgf.config.ExcelAffixLevelPoolRule`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelAffixLevelPoolRule {
    pub rule_id: i32,
    pub level_id: i32,
    pub affix_pool_ids: Vec<i32>,
}

/// `tgf.config.ExcelAffixMonsterPoolRule`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelAffixMonsterPoolRule {
    pub rule_id: i32,
    pub monster_id: i32,
    pub level_id: i32,
    pub affix_pool_ids: Vec<i32>,
}

/// `tgf.config.ExcelAffixPool`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelAffixPool {
    pub affix_pool_id: i32,
    pub attribute_weight: Option<DBattleAttributeOffset>,
}

/// `tgf.config.ExcelAffixTemplate`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelAffixTemplate {
    pub item_id: i32,
    pub data: Option<DBattleAffixOffset>,
    pub server_affix_score: Option<DBattleGearServerAffixScore>,
    pub affix_display: Option<DBattleAffixDisplayConfig>,
}

/// `tgf.config.ExcelAffixValueTemplate`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelAffixValueTemplate {
    pub affix_value_id: i32,
    pub attribute_type: i32,
    pub data: Option<DBattleAffixAttributeValueOffset>,
}

/// `tgf.config.ExcelBattleAbilitySetDescription`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelBattleAbilitySetDescription {
    pub ability_set_id: i32,
    pub level: i32,
    pub description: String,
    pub attribute: Option<DBattleAttributeOffset>,
}

/// `tgf.config.ExcelBattleAffixEffectRule`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelBattleAffixEffectRule {
    pub group_id: EnAffixCorruptionGroup,
    pub random_condition_id: EnAffixCorruptionType,
    pub id: i32,
    pub attribute_type: i32,
    pub effect: Option<BattleAffixEffectRuleEffect>,
}

/// `tgf.config.ExcelBattleAttributeDescription`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelBattleAttributeDescription {
    pub attribute_type: i32,
    pub name: String,
    pub display_order: i32,
    pub display_percentage: bool,
    pub role_display_order: i32,
    pub attribute_classify: i32,
    pub attribute_icon: String,
    pub major_attribute_rating: i32,
    pub minor_attribute_rating: i32,
}

/// `tgf.config.ExcelChatChannelType`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelChatChannelType {
    pub chat_channel_type: u32,
    pub max_log_count: u32,
    pub gc_log_count: u32,
    pub gc_expire_duration: String,
    pub memory_only: bool,
    pub banned_rule: Option<DIntKV>,
    pub banned_duration: i32,
    pub words_limit: i32,
}

/// `tgf.config.ExcelCompetitionLevel`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelCompetitionLevel {
    pub competition_level_id: i32,
    pub season_id: i32,
    pub time_limit: Option<DCompetitionSeasonTime>,
    pub detail: Option<DCompetitionSeasonDetail>,
    pub level_type: i32,
    pub module_id: i32,
    pub is_only_active_open: bool,
    pub settle_interface_type: DSettleInterFaceType,
    pub battle_affix_score_require: i32,
    pub on_product_env: bool,
    pub is_allow_mithril_force: bool,
    pub pvp_mode: DStatisticsPVPMode,
    pub matching_statistics_ids: Vec<i32>,
    pub battle_gear_rarity_require: Option<DExcelCompetitionLevelGearRarityRequire>,
    pub is_allow_challenge_model: bool,
}

/// `tgf.config.ExcelCraftingGroup`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelCraftingGroup {
    pub group_id: i32,
    pub recipes: Vec<i32>,
}

/// `tgf.config.ExcelCraftingRecipe`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelCraftingRecipe {
    pub id: i32,
    pub unlock_condition: Vec<DFunctionUnlockCondition>,
    pub tag: i32,
    pub item_type: i32,
    pub time_cost: i32,
    pub count_limt: i32,
    pub product: Vec<DItemOffset>,
    pub great_success_rate: i32,
    pub great_success_multiple: i32,
    pub item_cost: Vec<DItemOffset>,
    pub count_max_lmit: i32,
}

/// `tgf.config.ExcelCraftingTag`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelCraftingTag {
    pub tag_id: i32,
    pub craft_name: String,
}

/// `tgf.config.ExcelCuverTableSource`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelCuverTableSource {
    pub id: i32,
    pub source: String,
}

/// `tgf.config.ExcelDLCContent`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelDLCContent {
    pub app_id: i32,
    pub dlc_type: EnDLCType,
    pub dlc_icon: String,
    pub dlc_img: String,
    pub dlc_name: String,
}

/// `tgf.config.ExcelDLCPacket`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelDLCPacket {
    pub app_id: i32,
    pub item_id: i32,
    pub is_recycle: bool,
    pub item: Option<DItemOffset>,
}

/// `tgf.config.ExcelErrorCode`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelErrorCode {
    pub error_code: i32,
    pub popup_mode: EnErrorCodePopupMode,
    pub invisible: bool,
}

/// `tgf.config.ExcelFacade`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelFacade {
    pub id: i32,
    pub r#type: EnFacadeType,
    pub priority: i32,
    pub on_sales: bool,
    pub sales_time: String,
    pub worth_value: i32,
    pub name: String,
    pub descs: String,
    pub icon: String,
    pub show_red_dot: bool,
    pub rarity: i32,
    pub tips_icon: String,
    pub suit_id: i32,
    pub materials: Vec<EnGearEquipSubType>,
    pub put_third_sorts: Vec<EnFacadeSkinThirdSortType>,
    pub access_type: Option<FacadeAccessType>,
    pub use_description: String,
    pub video: String,
    pub marketplace_pic: String,
    pub cost_options: Vec<MarketNormalOptionCost>,
    pub xyz: Option<DExcelUeXYZ>,
    pub offset: Option<DExcelUeXYZ>,
    pub scale: f32,
    pub hidden_shadow: bool,
    pub quality_type: String,
    pub materials_shows: Vec<EnGearEquipSubType>,
    pub ue_skin_ids: Vec<i32>,
}

/// `tgf.config.ExcelFacadeSort`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelFacadeSort {
    pub third_sort: EnFacadeSkinThirdSortType,
    pub third_sort_priority: i32,
    pub second_sort: EnFacadeSkinSecondSortType,
    pub first_sort: EnFacadeSkinFirstSortType,
    pub name: String,
    pub icon: String,
    pub show: bool,
    pub original_resource_id: i32,
    pub xyz: Option<DExcelUeXYZ>,
    pub offset: Option<DExcelUeXYZ>,
    pub scale: f32,
}

/// `tgf.config.ExcelGearBaseItem`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelGearBaseItem {
    pub gear_sub_type: EnGearEquipSubType,
    pub id: i32,
}

/// `tgf.config.ExcelGearDismantle`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelGearDismantle {
    pub item_id: i32,
    pub reward: Vec<DItemOffset>,
}

/// `tgf.config.ExcelGearDurability`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelGearDurability {
    pub equip_type: EnGearEquipSubType,
    pub rarity: EnItemRarity,
    pub normal_loss: Option<DGearDurabilityLoss>,
    pub death_loss: Option<DGearDurabilityLoss>,
    pub repair_item_consume: Vec<DItemOffset>,
}

/// `tgf.config.ExcelGearGemGuide`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelGearGemGuide {
    pub id: i32,
    pub unlock_cost: Vec<DItemOffset>,
    pub unlock_costs: Vec<DConfigCostItem>,
    pub sort_priority: i32,
    pub inst_id: i32,
    pub unlock_condition_text: String,
    pub name_text: String,
}

/// `tgf.config.ExcelGearGemInst`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelGearGemInst {
    pub item_id: i32,
    pub shape_type: EnGearInlaidShapeType,
}

/// `tgf.config.ExcelGearHeirloomGuide`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelGearHeirloomGuide {
    pub id: i32,
    pub group_id: i32,
    pub type_id: i32,
    pub unlock_condition: Vec<i32>,
    pub unlock_cost: Vec<DItemOffset>,
    pub unlock_costs: Vec<DConfigCostItem>,
    pub instantiation_costs: Vec<DConfigCostItem>,
    pub inst_id: i32,
    pub sort_priority: i32,
    pub display_slot: Vec<EnGearInlaidShapeType>,
    pub unlock_condition_text: String,
    pub name_text: String,
    pub icon_path: String,
}

/// `tgf.config.ExcelGearInsuranceItem`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelGearInsuranceItem {
    pub rarity: EnItemRarity,
    pub id: i32,
    pub standard_rarity: EnItemRarity,
    pub standard_rate: i32,
    pub sp_rate: i32,
}

/// `tgf.config.ExcelGearSoulBoundCost`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelGearSoulBoundCost {
    pub count: i32,
    pub cost_item: Vec<DItemOffset>,
}

/// `tgf.config.ExcelGearSoulBoundWearPartRandom`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelGearSoulBoundWearPartRandom {
    pub group_id: i32,
    pub wear_part: EnItemWearingPart,
}

/// `tgf.config.ExcelGearTemplate`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelGearTemplate {
    pub item_id: i32,
    pub display: Option<GearDisplayConfig>,
    pub element: EnGearElement,
    pub durability: i32,
    pub skin_third_sort: EnFacadeSkinThirdSortType,
}

/// `tgf.config.ExcelImmaculateTrialGrade`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelImmaculateTrialGrade {
    pub grade_id: i32,
    pub activity_rule_id: i32,
    pub reached_cond: Option<DTrialLevelCond>,
    pub first_rewards: Vec<DItemOffset>,
    pub random_reward_id: i32,
    pub icon: String,
    pub str_desc: String,
    pub display_cond: Option<DTrialDisplayerCond>,
    pub display_quantity: i32,
    pub show_items: Vec<DItemOffset>,
    pub first_show_rewards: Vec<DItemOffset>,
}

/// `tgf.config.ExcelImmaculateTrialRandomRewards`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelImmaculateTrialRandomRewards {
    pub random_reward_id: i32,
    pub career_type_id: i32,
    pub random_pool_id: i32,
}

/// `tgf.config.ExcelImmaculateTrialRule`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelImmaculateTrialRule {
    pub activity_rule_id: i32,
    pub activity_end_tips_time: I64,
    pub activity_end_match_time: I64,
    pub activity_end_lock_time: I64,
    pub open_time: Vec<i32>,
    pub win_count: i32,
    pub failed_count: i32,
    pub cost_itmes: Vec<DItemOffset>,
    pub first_reward_mail_id: i32,
    pub random_reward_mail_id: i32,
    pub rank_id: i32,
}

/// `tgf.config.ExcelItemDropItem`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelItemDropItem {
    pub random_id: I64,
    pub item_id: i32,
    pub cnt: i32,
    pub level: i32,
    pub rate: i32,
}

/// `tgf.config.ExcelItemLootRate`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelItemLootRate {
    pub random_id: I64,
    pub unlucky_value: i32,
    pub unlucky_group: i32,
    pub corruption_id: EnAffixCorruptionType,
    pub corruption_rate: i32,
}

/// `tgf.config.ExcelItemMagicFound`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelItemMagicFound {
    pub level: i32,
    pub coefficient: i32,
}

/// `tgf.config.ExcelItemRouter`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelItemRouter {
    pub item_coming_enum: EnMarketType,
    pub item_type_enum: EnItemType,
    pub item_router: EnItemStorageType,
}

/// `tgf.config.ExcelItemType`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelItemType {
    pub type_id: i32,
    pub special_career_require: Vec<EnRoleCareerType>,
    pub level_require: i32,
    pub name: String,
    pub type_name: String,
    pub description: String,
    pub use_description: String,
    pub item_major_type: EnItemMajorType,
    pub item_minor_type: EnItemMinorType,
    pub skin_third_sort: EnFacadeSkinThirdSortType,
    pub use_action: Option<DItemUseAction>,
    pub system_repurchase_price: i32,
    pub bring_out_ingame_type: EnItemBringoutInGameLimitType,
    pub is_forbidden_bring_into_battle: bool,
    pub item_combat_value: i32,
    pub item_quest_value: i32,
    pub display_season: i32,
    pub loot_log: bool,
    pub game_loot_sort: i32,
    pub ue_source_type_id: i32,
}

/// `tgf.config.ExcelLevelType`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelLevelType {
    pub level_type: i32,
    pub match_making_pool_id: i32,
}

/// `tgf.config.ExcelLootChooseGroupRate`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelLootChooseGroupRate {
    pub loot_id: I64,
    pub random_ids: Vec<I64>,
    pub pr: Vec<i32>,
    pub condition: Vec<DExcelLootChooseGroupRateCondition>,
    pub default_rate: i32,
    pub max_rate: i32,
    pub rare_pool: Vec<LootRarePoolConfig>,
}

/// `tgf.config.ExcelLootDisableLootId`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelLootDisableLootId {
    pub loot_id: I64,
}

/// `tgf.config.ExcelLootRarePoolCntLimit`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelLootRarePoolCntLimit {
    pub item_id: i32,
    pub period_loot_limit_num: i32,
    pub period_interval: String,
}

/// `tgf.config.ExcelLootRarePoolItemRate`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelLootRarePoolItemRate {
    pub random_id: I64,
    pub item_id: i32,
    pub cnt: i32,
    pub rate: i32,
}

/// `tgf.config.ExcelLootSourceMap`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelLootSourceMap {
    pub name: String,
    pub id: i32,
}

/// `tgf.config.ExcelLootUeSourceChooseGroup`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelLootUeSourceChooseGroup {
    pub ue_loot_source_id: i32,
    pub map_id: i32,
    pub loot_type: i32,
    pub loot_ids: Vec<I64>,
    pub limit: i32,
}

/// `tgf.config.ExcelLootUnluckyValue`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelLootUnluckyValue {
    pub rule_id: EnLootUnluckyType,
    pub reward_score: i32,
    pub priority: i32,
    pub only_active_by_other: bool,
}

/// `tgf.config.ExcelLotteryAccessKey`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelLotteryAccessKey {
    pub type_id: i32,
    pub access_to_keys: Vec<DLotteryKeyAccess>,
}

/// `tgf.config.ExcelLotteryPool`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelLotteryPool {
    pub id: i32,
    pub access_key_type_id: i32,
    pub random_pool_id: i32,
    pub item_type: EnLotteryItemType,
}

/// `tgf.config.ExcelMailTemplate`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelMailTemplate {
    pub type_id: i32,
    pub config: Option<DMailTemplate>,
}

/// `tgf.config.ExcelMapDetail`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelMapDetail {
    pub map_id: i32,
    pub loot_map_id: i32,
    pub timeout: i32,
    pub map_config: Vec<ExcelMapDetailMapConfig>,
    pub max_fps: i32,
    pub waiting_room_timeout: i32,
    pub option_args: Vec<i32>,
    pub ds_fleet: String,
    pub settle_interface_type: DSettleInterFaceType,
    pub dynamic_option_args: Vec<DMapDynamicDSOption>,
}

/// `tgf.config.ExcelMapDetailMapConfig`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelMapDetailMapConfig {
    pub ue_path: String,
    pub room_type: EnDsBattleScenesType,
}

/// `tgf.config.ExcelMapExp`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelMapExp {
    pub map_id: i32,
    pub alive_time: I64,
    pub exp: i32,
    pub team_bonus: i32,
}

/// `tgf.config.ExcelMapSelect`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelMapSelect {
    pub map_name_id: String,
    pub map_id: i32,
    pub difficulty: i32,
    pub level_id: i32,
    pub level_type: i32,
    pub map_name: String,
    pub join_members: i32,
    pub map_res: String,
    pub map_select_bg_res: String,
    pub model_feature_ids: Vec<i32>,
    pub map_res_grey: String,
    pub map_select_bg_res_grey: String,
}

/// `tgf.config.ExcelMapSelectModelFeature`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelMapSelectModelFeature {
    pub model_feature_id: i32,
    pub icon_res: String,
    pub model_feature_name: String,
    pub model_feature_describe: String,
    pub r#type: i32,
}

/// `tgf.config.ExcelMarketInfo`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelMarketInfo {
    pub market_id: i32,
    pub data: Option<MarketInfo>,
}

/// `tgf.config.ExcelMarketPoorMan`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelMarketPoorMan {
    pub role_type_id: i32,
    pub poorman_discount_product_ids: Vec<i32>,
}

/// `tgf.config.ExcelMarketPopupVideo`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelMarketPopupVideo {
    pub popup_video_id: i32,
    pub data: Option<MarketPopupVideo>,
}

/// `tgf.config.ExcelMarketProductItem`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelMarketProductItem {
    pub product_id: i32,
    pub market_tab_id: i32,
    pub refresh_pool_id: i32,
    pub display_template: String,
    pub product: Option<MarketProductItem>,
}

/// `tgf.config.ExcelMarketRefreshPool`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelMarketRefreshPool {
    pub market_refresh_pool_id: i32,
    pub market_location_count: i32,
    pub refresh_pool_random_weight: i32,
    pub role_level_limit: i32,
    pub refresh_pool_type: EnRefreshPoolType,
}

/// `tgf.config.ExcelMarketSpecialCost`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelMarketSpecialCost {
    pub product_id: i32,
    pub sort_idx: i32,
    pub data: Option<MarketSpecialCost>,
}

/// `tgf.config.ExcelMarketTabInfo`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelMarketTabInfo {
    pub market_tab_id: i32,
    pub data: Option<MarketTabInfo>,
}

/// `tgf.config.ExcelMithrilForceItem`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelMithrilForceItem {
    pub id: i32,
    pub unlock_level: i32,
    pub tab_id: i32,
    pub description: String,
    pub wearing_count_limit: i32,
}

/// `tgf.config.ExcelMithrilForcePrestige`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelMithrilForcePrestige {
    pub level: i32,
    pub exp: I64,
    pub name: String,
    pub description: String,
}

/// `tgf.config.ExcelMithrilForceSettlement`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelMithrilForceSettlement {
    pub damage: I64,
    pub exp: I64,
}

/// `tgf.config.ExcelMithrilForceSettlementFactor`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelMithrilForceSettlementFactor {
    pub damage: I64,
    pub currency_active_coefficient: i32,
    pub exp_bottle_active_coefficient: i32,
    pub affix_active_coefficient: i32,
    pub potion_active_coefficient: i32,
    pub heirloom_item_coefficient: i32,
}

/// `tgf.config.ExcelModuleUnlockCircular`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelModuleUnlockCircular {
    pub id: i32,
    pub start_time: String,
    pub end_time: String,
    pub circular_queue: Vec<DModuleCircularData>,
}

/// `tgf.config.ExcelModuleUnlockType`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelModuleUnlockType {
    pub module_id: i32,
    pub module_name: EnUnlockModuleID,
    pub global_forbid: bool,
    pub unlock_date: String,
    pub daily_range_times: Vec<DConditionRuleRangeTime>,
    pub date_range_times: Vec<DConditionRuleRangeDatetime>,
    pub unlock_condition: Vec<DFunctionUnlockCondition>,
    pub ban_tip: String,
    pub forbid_tip: String,
    pub lock_tip: String,
    pub ban_mail: i32,
    pub permanently_ban_mail: i32,
    pub custom_ban_mail: i32,
    pub permanently_custom_ban_mail: i32,
}

/// `tgf.config.excel_origin_const_config`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelOriginConstConfig {
    pub key: String,
    pub value: String,
}

/// `tgf.config.ExcelQuestGroupList`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelQuestGroupList {
    pub id: i32,
    pub name: String,
    pub description: String,
    pub base_pic: String,
}

/// `tgf.config.ExcelQuestList`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelQuestList {
    pub id: i32,
    pub activity_id: i32,
    pub on: bool,
    pub random_pool_id: i32,
    pub random_pool_weight: i32,
    pub r#type: EnQuestType,
    pub display_type: EnQuestDisplayType,
    pub received_reset: bool,
    pub adventure_id: i32,
    pub delete_after_unlock: Vec<i32>,
    pub name: String,
    pub display_position: i32,
    pub base_pic: String,
    pub display_order: i32,
    pub display_group: i32,
    pub available_period: Option<DQuestAvailablePeriodType>,
    pub unlock_conditions: Vec<DQuestUnlockConditionItem>,
    pub common_condition: Option<DConditionBasicLimit>,
    pub progress_reset_period: Option<DQuestProgressResetInfo>,
    pub progress: Vec<DQuestConditionProgress>,
    pub rewards: Option<DQuestReward>,
    pub add_progress: i32,
    pub client_submit_type: i32,
    pub redirect_addr: String,
    pub steam_achievement_api: String,
    pub steam_statistic_api: String,
}

/// `tgf.config.ExcelQuestProgressCondition`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelQuestProgressCondition {
    pub id: i32,
    pub value: Option<QuestProgressConditionData>,
}

/// `tgf.config.ExcelQuestProgressType`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelQuestProgressType {
    pub id: i32,
    pub quest_condition_types: Vec<i32>,
    pub value_compare_type: EnQuestProgressValueCompareType,
}

/// `tgf.config.ExcelQuestTriggerEventType`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelQuestTriggerEventType {
    pub id: EnQuestTirggerType,
    pub unlock_condition_types: Vec<i32>,
    pub progress_types: Vec<EnQuestProgressType>,
}

/// `tgf.config.ExcelRankPeriodRewardPool`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelRankPeriodRewardPool {
    pub rank_reward_pool_id: i32,
    pub rank_or_score: u32,
    pub content: Option<DRankPeriodRewardPoolContent>,
    pub show_items: Vec<DItemOffset>,
}

/// `tgf.config.ExcelRankReportSetting`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelRankReportSetting {
    pub rank_type: u32,
    pub rank_instance_id: u32,
    pub type_id: i32,
    pub level_ids: Vec<i32>,
    pub data_type: i32,
    pub sort: i32,
}

/// `tgf.config.ExcelRankRule`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelRankRule {
    pub rank_type: u32,
    pub rank_instance_id: u32,
    pub content: Option<DRankRuleContent>,
    pub ban_id: i32,
    pub is_role_rank: bool,
    pub rank_show_type: i32,
}

/// `tgf.config.ExcelRoleDefaultEquipment`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelRoleDefaultEquipment {
    pub config_id: i32,
    pub item_id: i32,
    pub storage_type: EnItemStorageType,
    pub wear_part: EnItemWearingPart,
    pub count: i32,
}

/// `tgf.config.ExcelRoleFacePinchingLimit`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelRoleFacePinchingLimit {
    pub race: EnRoleRaceType,
    pub gender_type: EnRoleGenderType,
    pub face_ids: Vec<i32>,
    pub skincolor_ids: Vec<i32>,
    pub eyecolor_ids: Vec<i32>,
    pub hair_ids: Vec<i32>,
    pub beard_ids: Vec<i32>,
    pub haircolor_ids: Vec<i32>,
    pub tatoo_ids: Vec<i32>,
    pub tatoocolor_ids: Vec<i32>,
    pub scar_type_ids: Vec<i32>,
    pub makeup_type_ids: Vec<i32>,
    pub makeup_color_ids: Vec<i32>,
    pub template_ids: Vec<i32>,
}

/// `tgf.config.ExcelRoleHeadIcon`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelRoleHeadIcon {
    pub type_id: i32,
    pub gender_type: EnRoleGenderType,
    pub face_id: i32,
    pub skincolor_id: i32,
    pub head_icon: String,
}

/// `tgf.config.ExcelRoleLevelExp`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelRoleLevelExp {
    pub level: i32,
    pub max_exp: i32,
}

/// `tgf.config.ExcelRoleLevelUpgrade`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelRoleLevelUpgrade {
    pub role_type_id: i32,
    pub role_level: i32,
    pub role_attribute: Option<DBattleAttributeOffset>,
}

/// `tgf.config.ExcelRoleSkill`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelRoleSkill {
    pub skill_id: i32,
    pub ability_set_id: i32,
    pub skill_name: String,
    pub skill_descr: String,
    pub initial_skill_descr: String,
    pub skill_position: EnRoleSkillPositionType,
    pub display_order: i32,
    pub is_display: bool,
}

/// `tgf.config.ExcelRoleSkinType`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelRoleSkinType {
    pub skin_id: i32,
    pub skin_name: String,
}

/// `tgf.config.ExcelRoleType`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelRoleType {
    pub type_id: i32,
    pub meta: Option<DRoleMetaIndex>,
    pub create_default_item_config_ids: Vec<i32>,
    pub game_failed_default_item_config_ids: Vec<i32>,
    pub pvp_default_item_config_ids: Vec<i32>,
    pub environment: EnEnvironmentType,
    pub create_show_item_config_ids: Vec<i32>,
    pub create_limit: Option<DConditionBasicLimit>,
    pub order: i32,
    pub revival_default_weapon: i32,
    pub available_gender: Vec<EnRoleGenderType>,
    pub force: EnForceType,
    pub skin_ids: Vec<i32>,
    pub str_desc: String,
    pub str_advantages: String,
    pub str_disadvantages: String,
    pub role_initial_skill: Vec<DRoleSkillKeyPosition>,
    pub role_initial_passive_skill: Vec<i32>,
    pub role_skill: Vec<i32>,
    pub role_passive_skill: Vec<i32>,
    pub str_equipments: String,
    pub str_name: String,
    pub icon_head: String,
    pub icon_career: String,
    pub rotation: Option<DExcelUeXYZ>,
    pub unlock_time: String,
    pub cumulative_login_day: i32,
}

/// `tgf.config.ExcelSeasonRule`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelSeasonRule {
    pub season_id: i32,
    pub season_begin_time: String,
    pub preseason_begin_time: String,
    pub season_end_time: String,
    pub craft_group_id: i32,
    pub merchant_market_id: i32,
    pub mall_market_id: i32,
    pub special_market_id: i32,
    pub trade_market_id: Vec<i32>,
    pub logic_mail_major_type: i32,
}

/// `tgf.config.ExcelSkillCalculate`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelSkillCalculate {
    pub id: String,
    pub formula_lua_id: String,
    pub ge_key: String,
}

/// `tgf.config.ExcelSkillNumExchange`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelSkillNumExchange {
    pub id: String,
    pub value: String,
    pub ge_key: String,
    pub as_percent: bool,
    pub decimal: i32,
}

/// `tgf.config.ExcelTiers`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelTiers {
    pub tiers_id: i32,
    pub major_tiers_id: i32,
    pub next_tiers_score: i32,
    pub demotion_protection: i32,
    pub score_id: i32,
    pub season_id: i32,
    pub next_tiers_id: i32,
    pub need_require_levels: Vec<i32>,
    pub tiers_name: String,
    pub tiers_icon: String,
    pub tiers_bp: String,
    pub tiers_color: String,
    pub tiers_rarity: i32,
}

/// `tgf.config.ExcelTiersKillPlyaerRule`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelTiersKillPlyaerRule {
    pub tiers_diff: i32,
    pub score: i32,
}

/// `tgf.config.ExcelTiersShowGroup`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelTiersShowGroup {
    pub type_id: i32,
    pub name: String,
    pub group_id: i32,
}

/// `tgf.config.ExcelTradeCommodity`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelTradeCommodity {
    pub id: i32,
    pub item_id: i32,
    pub market_id: i32,
    pub price_def: i32,
    pub price_method: EnTradePriceMethod,
    pub match_method: EnTradeMatchMethod,
    pub price_locked: U64,
    pub is_on_shelf: bool,
    pub system_buy_allowed: bool,
    pub system_buy_price_coefficient: i32,
    pub system_buy_time: String,
    pub tax_percentage: i32,
    pub price_upper_limit: i32,
    pub price_lower_limit: i32,
    pub trade_amount_coefficient: i32,
    pub increse_percentage_limit: i32,
    pub decrease_percentage_limit: i32,
    pub sell_delay: i32,
    pub order_valid_time: i32,
    pub sale_count_limit: i32,
    pub purchase_count_limit: i32,
    pub publish_cost: i32,
    pub is_calculate_sp: bool,
    pub item_data_require_id: i32,
    pub force_save_db: bool,
}

/// `tgf.config.ExcelTradeItemRequire`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelTradeItemRequire {
    pub id: i32,
    pub require: Vec<EnTradeItemRequireType>,
    pub priority: i32,
}

/// `tgf.config.ExcelTradeMarket`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelTradeMarket {
    pub id: i32,
    pub return_price_difference: bool,
    pub valid_lower_limit: I64,
    pub po_amount_limit: i32,
    pub so_amount_limit: i32,
    pub currency_type: i32,
    pub valid_price: i32,
    pub random_item: bool,
    pub market_type: EnTradeMarketType,
    pub ticket_type: Vec<EnTradeTicketType>,
    pub publish_cost_currency_type: i32,
    pub bid_price_percentage: i32,
    pub calculate_pool_id: i32,
    pub recommend_id: i32,
}

/// `tgf.config.ExcelTradeStandardPrice`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelTradeStandardPrice {
    pub id: i32,
    pub update_interval: String,
}

/// `tgf.config.ExcelTradeValidPrice`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelTradeValidPrice {
    pub id: i32,
    pub sp_min: i32,
    pub sp_max: i32,
    pub range: i32,
}

/// `tgf.config.ExcelTradeValidTime`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExcelTradeValidTime {
    pub id: i32,
    pub po_valid_time: String,
    pub so_valid_time: String,
}

/// `tgf.FacadeAccessType`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FacadeAccessType {
    pub r#type: EnFacadeAccessType,
    pub url: String,
}

/// `tgf.GearDisplayConfig`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GearDisplayConfig {
    pub as_show: f32,
}

/// `tgf.config.LootRarePoolConfig`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LootRarePoolConfig {
    pub pool_id: i32,
    pub basic_rate: i32,
    pub min_rate: i32,
}

/// `tgf.MarketInfo`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MarketInfo {
    pub market_id: i32,
    pub name: String,
    pub display_order: i32,
    pub market_tab_ids: Vec<i32>,
    pub is_on_sale: bool,
    pub r#type: i32,
    pub shared_market_tab_ids: Vec<i32>,
}

/// `tgf.MarketNormalCost`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MarketNormalCost {
    pub cost_type: i32,
    pub cost_price: i32,
    pub origin_price: i32,
}

/// `tgf.MarketNormalOptionCost`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MarketNormalOptionCost {
    pub cost_items: Vec<MarketNormalCost>,
}

/// `tgf.MarketPopupVideo`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MarketPopupVideo {
    pub popup_video_id: i32,
    pub market_id: i32,
    pub begin_time: String,
    pub end_time: String,
    pub videopath: String,
}

/// `tgf.MarketProductItem`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MarketProductItem {
    pub product_id: i32,
    pub sku_id: String,
    pub market_tab_id: i32,
    pub display_order: i32,
    pub is_on_sale: bool,
    pub random_weight: i32,
    pub cost_options: Vec<MarketNormalOptionCost>,
    pub name: String,
    pub description: String,
    pub reward_items: Vec<DItemOffset>,
    pub icon: String,
    pub refresh_pool_id: i32,
    pub refresh_count: i32,
    pub is_disabled_auto_refresh: bool,
    pub is_special_cost: bool,
    pub auto_use_reward_items: bool,
    pub level_trigger: i32,
    pub market_type: i32,
    pub rarity: EnItemRarity,
    pub identified: bool,
    pub buy_start_timepoint: String,
    pub buy_end_timepoint: String,
    pub discount_start_timepoint: String,
    pub discount_end_timepoint: String,
    pub videopath: String,
}

/// `tgf.MarketSpecialCost`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MarketSpecialCost {
    pub stock: i32,
    pub cost_options: Vec<MarketNormalOptionCost>,
}

/// `tgf.MarketTabInfo`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MarketTabInfo {
    pub market_tab_id: i32,
    pub name: String,
    pub display_order: i32,
    pub random_count: i32,
    pub period: I64,
    pub start_timepoint: String,
    pub display_tab_id: i32,
}

/// `tgf.QuestProgressConditionData`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct QuestProgressConditionData {
    pub battle_which_role_type: i32,
    pub battle_which_level_type: I64,
    pub battle_which_level_id: I64,
    pub battle_which_scense_type: EnDsBattleScenesType,
    pub battle_is_in_team: bool,
    pub battle_game_over_type: EnDSPlayerActorStatusType,
    pub battle_scene_tag: EnBattleGameplayPlayerSceneTag,
    pub battle_for_ac_type: EnActivityType,
    pub battle_which_one_level_id: Option<QuestProgressConditionLevelIds>,
    pub player_kill_actor_lt: i32,
    pub player_kill_actor_not_lt: i32,
    pub player_kill_lt: i32,
    pub player_kill_not_lt: i32,
    pub player_got_damage_lt: i32,
    pub player_got_damage_not_lt: i32,
    pub player_caused_damage_lt: i32,
    pub player_caused_damage_not_lt: i32,
    pub player_caused_heal_lt: i32,
    pub player_caused_heal_not_lt: i32,
    pub player_caused_damage_for_actor_lt: i32,
    pub player_caused_damage_for_actor_not_lt: i32,
    pub player_caused_damage_percent_not_lt: i32,
    pub player_caused_heal_percent_not_lt: i32,
    pub player_kill_actor_each_not_lt: i32,
    pub player_death_each_lt: i32,
    pub player_match_point: bool,
    pub player_rival_win_qty_lt: i32,
    pub role_which_career: EnRoleCareerType,
    pub role_which_gender: EnRoleGenderType,
    pub role_which_race: EnRoleRaceType,
    pub role_history_max_level_less: I64,
    pub role_level: I64,
    pub role_created_time_gt_quest: I64,
    pub role_which_force: EnForceType,
    pub item_chg_main_type: EnItemChgMainType,
    pub item_chg_sub_type: Option<QuestProgressConditionItemChgSubType>,
    pub item_raity: Option<QuestProgressConditionItemRarity>,
    pub item_major_type: EnItemMajorType,
    pub item_chg_channel: i32,
    pub item_gem_shape_type: EnGearInlaidShapeType,
    pub total_quest_worth_less_than: I64,
    pub total_coin_less_than: Option<DCoin>,
    pub total_coin_not_lt: Option<DCoin>,
    pub total_coin_greater_than: Option<DCoin>,
    pub total_coin_not_gt: Option<DCoin>,
}

/// `tgf.QuestProgressConditionItemChgSubType`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct QuestProgressConditionItemChgSubType {
    pub values: Vec<EnItemChgSubType>,
}

/// `tgf.QuestProgressConditionItemRarity`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct QuestProgressConditionItemRarity {
    pub values: Vec<EnItemRarity>,
}

/// `tgf.QuestProgressConditionLevelIds`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct QuestProgressConditionLevelIds {
    pub values: Vec<I64>,
}

/// `tgf.config.UESourceAbilitySet`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UESourceAbilitySet {
    pub ue_source_id: i32,
    pub ue_path: String,
}

/// `tgf.config.UESourceItemType`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UESourceItemType {
    pub type_id: i32,
    pub position_cfg: Option<DItemPositionCfg>,
    pub ue_source_equipment_equip_id: i32,
    pub rarity: EnItemRarity,
    pub slot_type: EnSkinSlotType,
    pub category_type: EnSkinCategoryType,
    pub item_type: EnItemUeType,
    pub brush_data: Option<DUeSourceBrushData>,
    pub tips_brush_data: Option<DUeSourceBrushData>,
    pub ue_path: String,
}

/// `tgf.config.UeSourceActorSkin`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UeSourceActorSkin {
    pub skin_id: i32,
    pub slot_type: EnSkinSlotType,
    pub category_type: EnSkinCategoryType,
    pub ue_path: String,
}

/// `tgf.config.UeSourceCharacter`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UeSourceCharacter {
    pub role_id: i32,
    pub ue_path: String,
}

/// `tgf.config.UeSourceEquipment`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UeSourceEquipment {
    pub equip_id: i32,
    pub equip_subtype: EnGearEquipSubType,
    pub equipment_type: EnGearEquipmentType,
    pub weapon_configs: Vec<EquipmentWeaponConfig>,
    pub gear_configs: Vec<EquipmentGearConfig>,
    pub ue_path: String,
}

/// `tgf.config.UeSourceEquipmentSubType`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UeSourceEquipmentSubType {
    pub career_config: Vec<EquipmentSubTypeCareerConfig>,
    pub ue_path: String,
}

/// Every decoded config table, keyed by its JSON file name.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ConfigTables {
    /// `ExcelActivityCenter.json` (`tgf.config.ExcelActivityCenter`)
    pub excel_activity_center: Vec<ExcelActivityCenter>,
    /// `ExcelActivityCenterPassprotsReards.json` (`tgf.config.ExcelActivityCenterPassprotsReards`)
    pub excel_activity_center_passprots_reards: Vec<ExcelActivityCenterPassprotsReards>,
    /// `ExcelActivityCenterRandomRewards.json` (`tgf.config.ExcelActivityCenterRandomRewards`)
    pub excel_activity_center_random_rewards: Vec<ExcelActivityCenterRandomRewards>,
    /// `ExcelActivityCenterRandomRewardsPool.json` (`tgf.config.ExcelActivityCenterRandomRewardsPool`)
    pub excel_activity_center_random_rewards_pool: Vec<ExcelActivityCenterRandomRewardsPool>,
    /// `ExcelAffixAbilitySetId.json` (`tgf.config.ExcelAffixAbilitySetId`)
    pub excel_affix_ability_set_id: Vec<ExcelAffixAbilitySetId>,
    /// `ExcelAffixCountRandomPool.json` (`tgf.config.ExcelAffixCountRandomPool`)
    pub excel_affix_count_random_pool: Vec<ExcelAffixCountRandomPool>,
    /// `ExcelAffixGearSetMapRule.json` (`tgf.config.ExcelAffixGearSetMapRule`)
    pub excel_affix_gear_set_map_rule: Vec<ExcelAffixGearSetMapRule>,
    /// `ExcelAffixGearSetRule.json` (`tgf.config.ExcelAffixGearSetRule`)
    pub excel_affix_gear_set_rule: Vec<ExcelAffixGearSetRule>,
    /// `ExcelAffixLegendaryRule.json` (`tgf.config.ExcelAffixLegendaryRule`)
    pub excel_affix_legendary_rule: Vec<ExcelAffixLegendaryRule>,
    /// `ExcelAffixLevelPoolRule.json` (`tgf.config.ExcelAffixLevelPoolRule`)
    pub excel_affix_level_pool_rule: Vec<ExcelAffixLevelPoolRule>,
    /// `ExcelAffixMonsterPoolRule.json` (`tgf.config.ExcelAffixMonsterPoolRule`)
    pub excel_affix_monster_pool_rule: Vec<ExcelAffixMonsterPoolRule>,
    /// `ExcelAffixPool.json` (`tgf.config.ExcelAffixPool`)
    pub excel_affix_pool: Vec<ExcelAffixPool>,
    /// `ExcelAffixTemplate.json` (`tgf.config.ExcelAffixTemplate`)
    pub excel_affix_template: Vec<ExcelAffixTemplate>,
    /// `ExcelAffixValueTemplate.json` (`tgf.config.ExcelAffixValueTemplate`)
    pub excel_affix_value_template: Vec<ExcelAffixValueTemplate>,
    /// `ExcelBattleAbilitySetDescription.json` (`tgf.config.ExcelBattleAbilitySetDescription`)
    pub excel_battle_ability_set_description: Vec<ExcelBattleAbilitySetDescription>,
    /// `ExcelBattleAffixEffectRule.json` (`tgf.config.ExcelBattleAffixEffectRule`)
    pub excel_battle_affix_effect_rule: Vec<ExcelBattleAffixEffectRule>,
    /// `ExcelBattleAttributeDescription.json` (`tgf.config.ExcelBattleAttributeDescription`)
    pub excel_battle_attribute_description: Vec<ExcelBattleAttributeDescription>,
    /// `ExcelChatChannelType.json` (`tgf.config.ExcelChatChannelType`)
    pub excel_chat_channel_type: Vec<ExcelChatChannelType>,
    /// `ExcelCompetitionLevel.json` (`tgf.config.ExcelCompetitionLevel`)
    pub excel_competition_level: Vec<ExcelCompetitionLevel>,
    /// `ExcelCraftingGroup.json` (`tgf.config.ExcelCraftingGroup`)
    pub excel_crafting_group: Vec<ExcelCraftingGroup>,
    /// `ExcelCraftingRecipe.json` (`tgf.config.ExcelCraftingRecipe`)
    pub excel_crafting_recipe: Vec<ExcelCraftingRecipe>,
    /// `ExcelCraftingTag.json` (`tgf.config.ExcelCraftingTag`)
    pub excel_crafting_tag: Vec<ExcelCraftingTag>,
    /// `ExcelCuverTableSource.json` (`tgf.config.ExcelCuverTableSource`)
    pub excel_cuver_table_source: Vec<ExcelCuverTableSource>,
    /// `ExcelDLCContent.json` (`tgf.config.ExcelDLCContent`)
    pub excel_dlccontent: Vec<ExcelDLCContent>,
    /// `ExcelDLCPacket.json` (`tgf.config.ExcelDLCPacket`)
    pub excel_dlcpacket: Vec<ExcelDLCPacket>,
    /// `ExcelErrorCode.json` (`tgf.config.ExcelErrorCode`)
    pub excel_error_code: Vec<ExcelErrorCode>,
    /// `ExcelFacade.json` (`tgf.config.ExcelFacade`)
    pub excel_facade: Vec<ExcelFacade>,
    /// `ExcelFacadeSort.json` (`tgf.config.ExcelFacadeSort`)
    pub excel_facade_sort: Vec<ExcelFacadeSort>,
    /// `ExcelGearBaseItem.json` (`tgf.config.ExcelGearBaseItem`)
    pub excel_gear_base_item: Vec<ExcelGearBaseItem>,
    /// `ExcelGearDismantle.json` (`tgf.config.ExcelGearDismantle`)
    pub excel_gear_dismantle: Vec<ExcelGearDismantle>,
    /// `ExcelGearDurability.json` (`tgf.config.ExcelGearDurability`)
    pub excel_gear_durability: Vec<ExcelGearDurability>,
    /// `ExcelGearGemGuide.json` (`tgf.config.ExcelGearGemGuide`)
    pub excel_gear_gem_guide: Vec<ExcelGearGemGuide>,
    /// `ExcelGearGemInst.json` (`tgf.config.ExcelGearGemInst`)
    pub excel_gear_gem_inst: Vec<ExcelGearGemInst>,
    /// `ExcelGearHeirloomGuide.json` (`tgf.config.ExcelGearHeirloomGuide`)
    pub excel_gear_heirloom_guide: Vec<ExcelGearHeirloomGuide>,
    /// `ExcelGearInsuranceItem.json` (`tgf.config.ExcelGearInsuranceItem`)
    pub excel_gear_insurance_item: Vec<ExcelGearInsuranceItem>,
    /// `ExcelGearSoulBoundCost.json` (`tgf.config.ExcelGearSoulBoundCost`)
    pub excel_gear_soul_bound_cost: Vec<ExcelGearSoulBoundCost>,
    /// `ExcelGearSoulBoundWearPartRandom.json` (`tgf.config.ExcelGearSoulBoundWearPartRandom`)
    pub excel_gear_soul_bound_wear_part_random: Vec<ExcelGearSoulBoundWearPartRandom>,
    /// `ExcelGearTemplate.json` (`tgf.config.ExcelGearTemplate`)
    pub excel_gear_template: Vec<ExcelGearTemplate>,
    /// `ExcelImmaculateTrialGrade.json` (`tgf.config.ExcelImmaculateTrialGrade`)
    pub excel_immaculate_trial_grade: Vec<ExcelImmaculateTrialGrade>,
    /// `ExcelImmaculateTrialRandomRewards.json` (`tgf.config.ExcelImmaculateTrialRandomRewards`)
    pub excel_immaculate_trial_random_rewards: Vec<ExcelImmaculateTrialRandomRewards>,
    /// `ExcelImmaculateTrialRule.json` (`tgf.config.ExcelImmaculateTrialRule`)
    pub excel_immaculate_trial_rule: Vec<ExcelImmaculateTrialRule>,
    /// `ExcelItemDropItem.json` (`tgf.config.ExcelItemDropItem`)
    pub excel_item_drop_item: Vec<ExcelItemDropItem>,
    /// `ExcelItemLootRate.json` (`tgf.config.ExcelItemLootRate`)
    pub excel_item_loot_rate: Vec<ExcelItemLootRate>,
    /// `ExcelItemMagicFound.json` (`tgf.config.ExcelItemMagicFound`)
    pub excel_item_magic_found: Vec<ExcelItemMagicFound>,
    /// `ExcelItemRouter.json` (`tgf.config.ExcelItemRouter`)
    pub excel_item_router: Vec<ExcelItemRouter>,
    /// `ExcelItemType.json` (`tgf.config.ExcelItemType`)
    pub excel_item_type: Vec<ExcelItemType>,
    /// `ExcelLevelType.json` (`tgf.config.ExcelLevelType`)
    pub excel_level_type: Vec<ExcelLevelType>,
    /// `ExcelLootChooseGroupRate.json` (`tgf.config.ExcelLootChooseGroupRate`)
    pub excel_loot_choose_group_rate: Vec<ExcelLootChooseGroupRate>,
    /// `ExcelLootDisableLootId.json` (`tgf.config.ExcelLootDisableLootId`)
    pub excel_loot_disable_loot_id: Vec<ExcelLootDisableLootId>,
    /// `ExcelLootRarePoolCntLimit.json` (`tgf.config.ExcelLootRarePoolCntLimit`)
    pub excel_loot_rare_pool_cnt_limit: Vec<ExcelLootRarePoolCntLimit>,
    /// `ExcelLootRarePoolItemRate.json` (`tgf.config.ExcelLootRarePoolItemRate`)
    pub excel_loot_rare_pool_item_rate: Vec<ExcelLootRarePoolItemRate>,
    /// `ExcelLootSourceMap.json` (`tgf.config.ExcelLootSourceMap`)
    pub excel_loot_source_map: Vec<ExcelLootSourceMap>,
    /// `ExcelLootUeSourceChooseGroup.json` (`tgf.config.ExcelLootUeSourceChooseGroup`)
    pub excel_loot_ue_source_choose_group: Vec<ExcelLootUeSourceChooseGroup>,
    /// `ExcelLootUnluckyValue.json` (`tgf.config.ExcelLootUnluckyValue`)
    pub excel_loot_unlucky_value: Vec<ExcelLootUnluckyValue>,
    /// `ExcelLotteryAccessKey.json` (`tgf.config.ExcelLotteryAccessKey`)
    pub excel_lottery_access_key: Vec<ExcelLotteryAccessKey>,
    /// `ExcelLotteryPool.json` (`tgf.config.ExcelLotteryPool`)
    pub excel_lottery_pool: Vec<ExcelLotteryPool>,
    /// `ExcelMailTemplate.json` (`tgf.config.ExcelMailTemplate`)
    pub excel_mail_template: Vec<ExcelMailTemplate>,
    /// `ExcelMapDetail.json` (`tgf.config.ExcelMapDetail`)
    pub excel_map_detail: Vec<ExcelMapDetail>,
    /// `ExcelMapExp.json` (`tgf.config.ExcelMapExp`)
    pub excel_map_exp: Vec<ExcelMapExp>,
    /// `ExcelMapSelect.json` (`tgf.config.ExcelMapSelect`)
    pub excel_map_select: Vec<ExcelMapSelect>,
    /// `ExcelMapSelectModelFeature.json` (`tgf.config.ExcelMapSelectModelFeature`)
    pub excel_map_select_model_feature: Vec<ExcelMapSelectModelFeature>,
    /// `ExcelMarketInfo.json` (`tgf.config.ExcelMarketInfo`)
    pub excel_market_info: Vec<ExcelMarketInfo>,
    /// `ExcelMarketPoorMan.json` (`tgf.config.ExcelMarketPoorMan`)
    pub excel_market_poor_man: Vec<ExcelMarketPoorMan>,
    /// `ExcelMarketPopupVideo.json` (`tgf.config.ExcelMarketPopupVideo`)
    pub excel_market_popup_video: Vec<ExcelMarketPopupVideo>,
    /// `ExcelMarketProductItem.json` (`tgf.config.ExcelMarketProductItem`)
    pub excel_market_product_item: Vec<ExcelMarketProductItem>,
    /// `ExcelMarketRefreshPool.json` (`tgf.config.ExcelMarketRefreshPool`)
    pub excel_market_refresh_pool: Vec<ExcelMarketRefreshPool>,
    /// `ExcelMarketSpecialCost.json` (`tgf.config.ExcelMarketSpecialCost`)
    pub excel_market_special_cost: Vec<ExcelMarketSpecialCost>,
    /// `ExcelMarketTabInfo.json` (`tgf.config.ExcelMarketTabInfo`)
    pub excel_market_tab_info: Vec<ExcelMarketTabInfo>,
    /// `ExcelMithrilForceItem.json` (`tgf.config.ExcelMithrilForceItem`)
    pub excel_mithril_force_item: Vec<ExcelMithrilForceItem>,
    /// `ExcelMithrilForcePrestige.json` (`tgf.config.ExcelMithrilForcePrestige`)
    pub excel_mithril_force_prestige: Vec<ExcelMithrilForcePrestige>,
    /// `ExcelMithrilForceSettlement.json` (`tgf.config.ExcelMithrilForceSettlement`)
    pub excel_mithril_force_settlement: Vec<ExcelMithrilForceSettlement>,
    /// `ExcelMithrilForceSettlementFactor.json` (`tgf.config.ExcelMithrilForceSettlementFactor`)
    pub excel_mithril_force_settlement_factor: Vec<ExcelMithrilForceSettlementFactor>,
    /// `ExcelModuleUnlockCircular.json` (`tgf.config.ExcelModuleUnlockCircular`)
    pub excel_module_unlock_circular: Vec<ExcelModuleUnlockCircular>,
    /// `ExcelModuleUnlockType.json` (`tgf.config.ExcelModuleUnlockType`)
    pub excel_module_unlock_type: Vec<ExcelModuleUnlockType>,
    /// `ExcelQuestGroupList.json` (`tgf.config.ExcelQuestGroupList`)
    pub excel_quest_group_list: Vec<ExcelQuestGroupList>,
    /// `ExcelQuestList.json` (`tgf.config.ExcelQuestList`)
    pub excel_quest_list: Vec<ExcelQuestList>,
    /// `ExcelQuestProgressCondition.json` (`tgf.config.ExcelQuestProgressCondition`)
    pub excel_quest_progress_condition: Vec<ExcelQuestProgressCondition>,
    /// `ExcelQuestProgressType.json` (`tgf.config.ExcelQuestProgressType`)
    pub excel_quest_progress_type: Vec<ExcelQuestProgressType>,
    /// `ExcelQuestTriggerEventType.json` (`tgf.config.ExcelQuestTriggerEventType`)
    pub excel_quest_trigger_event_type: Vec<ExcelQuestTriggerEventType>,
    /// `ExcelRankPeriodRewardPool.json` (`tgf.config.ExcelRankPeriodRewardPool`)
    pub excel_rank_period_reward_pool: Vec<ExcelRankPeriodRewardPool>,
    /// `ExcelRankReportSetting.json` (`tgf.config.ExcelRankReportSetting`)
    pub excel_rank_report_setting: Vec<ExcelRankReportSetting>,
    /// `ExcelRankRule.json` (`tgf.config.ExcelRankRule`)
    pub excel_rank_rule: Vec<ExcelRankRule>,
    /// `ExcelRoleDefaultEquipment.json` (`tgf.config.ExcelRoleDefaultEquipment`)
    pub excel_role_default_equipment: Vec<ExcelRoleDefaultEquipment>,
    /// `ExcelRoleFacePinchingLimit.json` (`tgf.config.ExcelRoleFacePinchingLimit`)
    pub excel_role_face_pinching_limit: Vec<ExcelRoleFacePinchingLimit>,
    /// `ExcelRoleHeadIcon.json` (`tgf.config.ExcelRoleHeadIcon`)
    pub excel_role_head_icon: Vec<ExcelRoleHeadIcon>,
    /// `ExcelRoleLevelExp.json` (`tgf.config.ExcelRoleLevelExp`)
    pub excel_role_level_exp: Vec<ExcelRoleLevelExp>,
    /// `ExcelRoleLevelUpgrade.json` (`tgf.config.ExcelRoleLevelUpgrade`)
    pub excel_role_level_upgrade: Vec<ExcelRoleLevelUpgrade>,
    /// `ExcelRoleSkill.json` (`tgf.config.ExcelRoleSkill`)
    pub excel_role_skill: Vec<ExcelRoleSkill>,
    /// `ExcelRoleSkinType.json` (`tgf.config.ExcelRoleSkinType`)
    pub excel_role_skin_type: Vec<ExcelRoleSkinType>,
    /// `ExcelRoleType.json` (`tgf.config.ExcelRoleType`)
    pub excel_role_type: Vec<ExcelRoleType>,
    /// `ExcelSeasonRule.json` (`tgf.config.ExcelSeasonRule`)
    pub excel_season_rule: Vec<ExcelSeasonRule>,
    /// `ExcelSkillCalculate.json` (`tgf.config.ExcelSkillCalculate`)
    pub excel_skill_calculate: Vec<ExcelSkillCalculate>,
    /// `ExcelSkillNumExchange.json` (`tgf.config.ExcelSkillNumExchange`)
    pub excel_skill_num_exchange: Vec<ExcelSkillNumExchange>,
    /// `ExcelTiers.json` (`tgf.config.ExcelTiers`)
    pub excel_tiers: Vec<ExcelTiers>,
    /// `ExcelTiersKillPlyaerRule.json` (`tgf.config.ExcelTiersKillPlyaerRule`)
    pub excel_tiers_kill_plyaer_rule: Vec<ExcelTiersKillPlyaerRule>,
    /// `ExcelTiersShowGroup.json` (`tgf.config.ExcelTiersShowGroup`)
    pub excel_tiers_show_group: Vec<ExcelTiersShowGroup>,
    /// `ExcelTradeCommodity.json` (`tgf.config.ExcelTradeCommodity`)
    pub excel_trade_commodity: Vec<ExcelTradeCommodity>,
    /// `ExcelTradeItemRequire.json` (`tgf.config.ExcelTradeItemRequire`)
    pub excel_trade_item_require: Vec<ExcelTradeItemRequire>,
    /// `ExcelTradeMarket.json` (`tgf.config.ExcelTradeMarket`)
    pub excel_trade_market: Vec<ExcelTradeMarket>,
    /// `ExcelTradeStandardPrice.json` (`tgf.config.ExcelTradeStandardPrice`)
    pub excel_trade_standard_price: Vec<ExcelTradeStandardPrice>,
    /// `ExcelTradeValidPrice.json` (`tgf.config.ExcelTradeValidPrice`)
    pub excel_trade_valid_price: Vec<ExcelTradeValidPrice>,
    /// `ExcelTradeValidTime.json` (`tgf.config.ExcelTradeValidTime`)
    pub excel_trade_valid_time: Vec<ExcelTradeValidTime>,
    /// `UESourceAbilitySet.json` (`tgf.config.UESourceAbilitySet`)
    pub uesource_ability_set: Vec<UESourceAbilitySet>,
    /// `UESourceItemType.json` (`tgf.config.UESourceItemType`)
    pub uesource_item_type: Vec<UESourceItemType>,
    /// `UeSourceActorSkin.json` (`tgf.config.UeSourceActorSkin`)
    pub ue_source_actor_skin: Vec<UeSourceActorSkin>,
    /// `UeSourceCharacter.json` (`tgf.config.UeSourceCharacter`)
    pub ue_source_character: Vec<UeSourceCharacter>,
    /// `UeSourceEquipment.json` (`tgf.config.UeSourceEquipment`)
    pub ue_source_equipment: Vec<UeSourceEquipment>,
    /// `UeSourceEquipmentSubType.json` (`tgf.config.UeSourceEquipmentSubType`)
    pub ue_source_equipment_sub_type: Vec<UeSourceEquipmentSubType>,
    /// `excel_origin_const_config.json` (`tgf.config.excel_origin_const_config`)
    pub excel_origin_const_config: Vec<ExcelOriginConstConfig>,
}

/// (JSON file name, protobuf message type) for every table in [`ConfigTables`].
pub const TABLES: [(&str, &str); 109] = [
    ("ExcelActivityCenter", "tgf.config.ExcelActivityCenter"),
    ("ExcelActivityCenterPassprotsReards", "tgf.config.ExcelActivityCenterPassprotsReards"),
    ("ExcelActivityCenterRandomRewards", "tgf.config.ExcelActivityCenterRandomRewards"),
    ("ExcelActivityCenterRandomRewardsPool", "tgf.config.ExcelActivityCenterRandomRewardsPool"),
    ("ExcelAffixAbilitySetId", "tgf.config.ExcelAffixAbilitySetId"),
    ("ExcelAffixCountRandomPool", "tgf.config.ExcelAffixCountRandomPool"),
    ("ExcelAffixGearSetMapRule", "tgf.config.ExcelAffixGearSetMapRule"),
    ("ExcelAffixGearSetRule", "tgf.config.ExcelAffixGearSetRule"),
    ("ExcelAffixLegendaryRule", "tgf.config.ExcelAffixLegendaryRule"),
    ("ExcelAffixLevelPoolRule", "tgf.config.ExcelAffixLevelPoolRule"),
    ("ExcelAffixMonsterPoolRule", "tgf.config.ExcelAffixMonsterPoolRule"),
    ("ExcelAffixPool", "tgf.config.ExcelAffixPool"),
    ("ExcelAffixTemplate", "tgf.config.ExcelAffixTemplate"),
    ("ExcelAffixValueTemplate", "tgf.config.ExcelAffixValueTemplate"),
    ("ExcelBattleAbilitySetDescription", "tgf.config.ExcelBattleAbilitySetDescription"),
    ("ExcelBattleAffixEffectRule", "tgf.config.ExcelBattleAffixEffectRule"),
    ("ExcelBattleAttributeDescription", "tgf.config.ExcelBattleAttributeDescription"),
    ("ExcelChatChannelType", "tgf.config.ExcelChatChannelType"),
    ("ExcelCompetitionLevel", "tgf.config.ExcelCompetitionLevel"),
    ("ExcelCraftingGroup", "tgf.config.ExcelCraftingGroup"),
    ("ExcelCraftingRecipe", "tgf.config.ExcelCraftingRecipe"),
    ("ExcelCraftingTag", "tgf.config.ExcelCraftingTag"),
    ("ExcelCuverTableSource", "tgf.config.ExcelCuverTableSource"),
    ("ExcelDLCContent", "tgf.config.ExcelDLCContent"),
    ("ExcelDLCPacket", "tgf.config.ExcelDLCPacket"),
    ("ExcelErrorCode", "tgf.config.ExcelErrorCode"),
    ("ExcelFacade", "tgf.config.ExcelFacade"),
    ("ExcelFacadeSort", "tgf.config.ExcelFacadeSort"),
    ("ExcelGearBaseItem", "tgf.config.ExcelGearBaseItem"),
    ("ExcelGearDismantle", "tgf.config.ExcelGearDismantle"),
    ("ExcelGearDurability", "tgf.config.ExcelGearDurability"),
    ("ExcelGearGemGuide", "tgf.config.ExcelGearGemGuide"),
    ("ExcelGearGemInst", "tgf.config.ExcelGearGemInst"),
    ("ExcelGearHeirloomGuide", "tgf.config.ExcelGearHeirloomGuide"),
    ("ExcelGearInsuranceItem", "tgf.config.ExcelGearInsuranceItem"),
    ("ExcelGearSoulBoundCost", "tgf.config.ExcelGearSoulBoundCost"),
    ("ExcelGearSoulBoundWearPartRandom", "tgf.config.ExcelGearSoulBoundWearPartRandom"),
    ("ExcelGearTemplate", "tgf.config.ExcelGearTemplate"),
    ("ExcelImmaculateTrialGrade", "tgf.config.ExcelImmaculateTrialGrade"),
    ("ExcelImmaculateTrialRandomRewards", "tgf.config.ExcelImmaculateTrialRandomRewards"),
    ("ExcelImmaculateTrialRule", "tgf.config.ExcelImmaculateTrialRule"),
    ("ExcelItemDropItem", "tgf.config.ExcelItemDropItem"),
    ("ExcelItemLootRate", "tgf.config.ExcelItemLootRate"),
    ("ExcelItemMagicFound", "tgf.config.ExcelItemMagicFound"),
    ("ExcelItemRouter", "tgf.config.ExcelItemRouter"),
    ("ExcelItemType", "tgf.config.ExcelItemType"),
    ("ExcelLevelType", "tgf.config.ExcelLevelType"),
    ("ExcelLootChooseGroupRate", "tgf.config.ExcelLootChooseGroupRate"),
    ("ExcelLootDisableLootId", "tgf.config.ExcelLootDisableLootId"),
    ("ExcelLootRarePoolCntLimit", "tgf.config.ExcelLootRarePoolCntLimit"),
    ("ExcelLootRarePoolItemRate", "tgf.config.ExcelLootRarePoolItemRate"),
    ("ExcelLootSourceMap", "tgf.config.ExcelLootSourceMap"),
    ("ExcelLootUeSourceChooseGroup", "tgf.config.ExcelLootUeSourceChooseGroup"),
    ("ExcelLootUnluckyValue", "tgf.config.ExcelLootUnluckyValue"),
    ("ExcelLotteryAccessKey", "tgf.config.ExcelLotteryAccessKey"),
    ("ExcelLotteryPool", "tgf.config.ExcelLotteryPool"),
    ("ExcelMailTemplate", "tgf.config.ExcelMailTemplate"),
    ("ExcelMapDetail", "tgf.config.ExcelMapDetail"),
    ("ExcelMapExp", "tgf.config.ExcelMapExp"),
    ("ExcelMapSelect", "tgf.config.ExcelMapSelect"),
    ("ExcelMapSelectModelFeature", "tgf.config.ExcelMapSelectModelFeature"),
    ("ExcelMarketInfo", "tgf.config.ExcelMarketInfo"),
    ("ExcelMarketPoorMan", "tgf.config.ExcelMarketPoorMan"),
    ("ExcelMarketPopupVideo", "tgf.config.ExcelMarketPopupVideo"),
    ("ExcelMarketProductItem", "tgf.config.ExcelMarketProductItem"),
    ("ExcelMarketRefreshPool", "tgf.config.ExcelMarketRefreshPool"),
    ("ExcelMarketSpecialCost", "tgf.config.ExcelMarketSpecialCost"),
    ("ExcelMarketTabInfo", "tgf.config.ExcelMarketTabInfo"),
    ("ExcelMithrilForceItem", "tgf.config.ExcelMithrilForceItem"),
    ("ExcelMithrilForcePrestige", "tgf.config.ExcelMithrilForcePrestige"),
    ("ExcelMithrilForceSettlement", "tgf.config.ExcelMithrilForceSettlement"),
    ("ExcelMithrilForceSettlementFactor", "tgf.config.ExcelMithrilForceSettlementFactor"),
    ("ExcelModuleUnlockCircular", "tgf.config.ExcelModuleUnlockCircular"),
    ("ExcelModuleUnlockType", "tgf.config.ExcelModuleUnlockType"),
    ("ExcelQuestGroupList", "tgf.config.ExcelQuestGroupList"),
    ("ExcelQuestList", "tgf.config.ExcelQuestList"),
    ("ExcelQuestProgressCondition", "tgf.config.ExcelQuestProgressCondition"),
    ("ExcelQuestProgressType", "tgf.config.ExcelQuestProgressType"),
    ("ExcelQuestTriggerEventType", "tgf.config.ExcelQuestTriggerEventType"),
    ("ExcelRankPeriodRewardPool", "tgf.config.ExcelRankPeriodRewardPool"),
    ("ExcelRankReportSetting", "tgf.config.ExcelRankReportSetting"),
    ("ExcelRankRule", "tgf.config.ExcelRankRule"),
    ("ExcelRoleDefaultEquipment", "tgf.config.ExcelRoleDefaultEquipment"),
    ("ExcelRoleFacePinchingLimit", "tgf.config.ExcelRoleFacePinchingLimit"),
    ("ExcelRoleHeadIcon", "tgf.config.ExcelRoleHeadIcon"),
    ("ExcelRoleLevelExp", "tgf.config.ExcelRoleLevelExp"),
    ("ExcelRoleLevelUpgrade", "tgf.config.ExcelRoleLevelUpgrade"),
    ("ExcelRoleSkill", "tgf.config.ExcelRoleSkill"),
    ("ExcelRoleSkinType", "tgf.config.ExcelRoleSkinType"),
    ("ExcelRoleType", "tgf.config.ExcelRoleType"),
    ("ExcelSeasonRule", "tgf.config.ExcelSeasonRule"),
    ("ExcelSkillCalculate", "tgf.config.ExcelSkillCalculate"),
    ("ExcelSkillNumExchange", "tgf.config.ExcelSkillNumExchange"),
    ("ExcelTiers", "tgf.config.ExcelTiers"),
    ("ExcelTiersKillPlyaerRule", "tgf.config.ExcelTiersKillPlyaerRule"),
    ("ExcelTiersShowGroup", "tgf.config.ExcelTiersShowGroup"),
    ("ExcelTradeCommodity", "tgf.config.ExcelTradeCommodity"),
    ("ExcelTradeItemRequire", "tgf.config.ExcelTradeItemRequire"),
    ("ExcelTradeMarket", "tgf.config.ExcelTradeMarket"),
    ("ExcelTradeStandardPrice", "tgf.config.ExcelTradeStandardPrice"),
    ("ExcelTradeValidPrice", "tgf.config.ExcelTradeValidPrice"),
    ("ExcelTradeValidTime", "tgf.config.ExcelTradeValidTime"),
    ("UESourceAbilitySet", "tgf.config.UESourceAbilitySet"),
    ("UESourceItemType", "tgf.config.UESourceItemType"),
    ("UeSourceActorSkin", "tgf.config.UeSourceActorSkin"),
    ("UeSourceCharacter", "tgf.config.UeSourceCharacter"),
    ("UeSourceEquipment", "tgf.config.UeSourceEquipment"),
    ("UeSourceEquipmentSubType", "tgf.config.UeSourceEquipmentSubType"),
    ("excel_origin_const_config", "tgf.config.excel_origin_const_config"),
];

impl ConfigTables {
    /// Load every table from `dir` (`<name>.json` each). Fails on the first table that does not parse.
    pub fn load(dir: &std::path::Path) -> anyhow::Result<Self> {
        Ok(Self {
            excel_activity_center: super::ConfigTable::load(dir, "ExcelActivityCenter")?.rows,
            excel_activity_center_passprots_reards: super::ConfigTable::load(dir, "ExcelActivityCenterPassprotsReards")?.rows,
            excel_activity_center_random_rewards: super::ConfigTable::load(dir, "ExcelActivityCenterRandomRewards")?.rows,
            excel_activity_center_random_rewards_pool: super::ConfigTable::load(dir, "ExcelActivityCenterRandomRewardsPool")?.rows,
            excel_affix_ability_set_id: super::ConfigTable::load(dir, "ExcelAffixAbilitySetId")?.rows,
            excel_affix_count_random_pool: super::ConfigTable::load(dir, "ExcelAffixCountRandomPool")?.rows,
            excel_affix_gear_set_map_rule: super::ConfigTable::load(dir, "ExcelAffixGearSetMapRule")?.rows,
            excel_affix_gear_set_rule: super::ConfigTable::load(dir, "ExcelAffixGearSetRule")?.rows,
            excel_affix_legendary_rule: super::ConfigTable::load(dir, "ExcelAffixLegendaryRule")?.rows,
            excel_affix_level_pool_rule: super::ConfigTable::load(dir, "ExcelAffixLevelPoolRule")?.rows,
            excel_affix_monster_pool_rule: super::ConfigTable::load(dir, "ExcelAffixMonsterPoolRule")?.rows,
            excel_affix_pool: super::ConfigTable::load(dir, "ExcelAffixPool")?.rows,
            excel_affix_template: super::ConfigTable::load(dir, "ExcelAffixTemplate")?.rows,
            excel_affix_value_template: super::ConfigTable::load(dir, "ExcelAffixValueTemplate")?.rows,
            excel_battle_ability_set_description: super::ConfigTable::load(dir, "ExcelBattleAbilitySetDescription")?.rows,
            excel_battle_affix_effect_rule: super::ConfigTable::load(dir, "ExcelBattleAffixEffectRule")?.rows,
            excel_battle_attribute_description: super::ConfigTable::load(dir, "ExcelBattleAttributeDescription")?.rows,
            excel_chat_channel_type: super::ConfigTable::load(dir, "ExcelChatChannelType")?.rows,
            excel_competition_level: super::ConfigTable::load(dir, "ExcelCompetitionLevel")?.rows,
            excel_crafting_group: super::ConfigTable::load(dir, "ExcelCraftingGroup")?.rows,
            excel_crafting_recipe: super::ConfigTable::load(dir, "ExcelCraftingRecipe")?.rows,
            excel_crafting_tag: super::ConfigTable::load(dir, "ExcelCraftingTag")?.rows,
            excel_cuver_table_source: super::ConfigTable::load(dir, "ExcelCuverTableSource")?.rows,
            excel_dlccontent: super::ConfigTable::load(dir, "ExcelDLCContent")?.rows,
            excel_dlcpacket: super::ConfigTable::load(dir, "ExcelDLCPacket")?.rows,
            excel_error_code: super::ConfigTable::load(dir, "ExcelErrorCode")?.rows,
            excel_facade: super::ConfigTable::load(dir, "ExcelFacade")?.rows,
            excel_facade_sort: super::ConfigTable::load(dir, "ExcelFacadeSort")?.rows,
            excel_gear_base_item: super::ConfigTable::load(dir, "ExcelGearBaseItem")?.rows,
            excel_gear_dismantle: super::ConfigTable::load(dir, "ExcelGearDismantle")?.rows,
            excel_gear_durability: super::ConfigTable::load(dir, "ExcelGearDurability")?.rows,
            excel_gear_gem_guide: super::ConfigTable::load(dir, "ExcelGearGemGuide")?.rows,
            excel_gear_gem_inst: super::ConfigTable::load(dir, "ExcelGearGemInst")?.rows,
            excel_gear_heirloom_guide: super::ConfigTable::load(dir, "ExcelGearHeirloomGuide")?.rows,
            excel_gear_insurance_item: super::ConfigTable::load(dir, "ExcelGearInsuranceItem")?.rows,
            excel_gear_soul_bound_cost: super::ConfigTable::load(dir, "ExcelGearSoulBoundCost")?.rows,
            excel_gear_soul_bound_wear_part_random: super::ConfigTable::load(dir, "ExcelGearSoulBoundWearPartRandom")?.rows,
            excel_gear_template: super::ConfigTable::load(dir, "ExcelGearTemplate")?.rows,
            excel_immaculate_trial_grade: super::ConfigTable::load(dir, "ExcelImmaculateTrialGrade")?.rows,
            excel_immaculate_trial_random_rewards: super::ConfigTable::load(dir, "ExcelImmaculateTrialRandomRewards")?.rows,
            excel_immaculate_trial_rule: super::ConfigTable::load(dir, "ExcelImmaculateTrialRule")?.rows,
            excel_item_drop_item: super::ConfigTable::load(dir, "ExcelItemDropItem")?.rows,
            excel_item_loot_rate: super::ConfigTable::load(dir, "ExcelItemLootRate")?.rows,
            excel_item_magic_found: super::ConfigTable::load(dir, "ExcelItemMagicFound")?.rows,
            excel_item_router: super::ConfigTable::load(dir, "ExcelItemRouter")?.rows,
            excel_item_type: super::ConfigTable::load(dir, "ExcelItemType")?.rows,
            excel_level_type: super::ConfigTable::load(dir, "ExcelLevelType")?.rows,
            excel_loot_choose_group_rate: super::ConfigTable::load(dir, "ExcelLootChooseGroupRate")?.rows,
            excel_loot_disable_loot_id: super::ConfigTable::load(dir, "ExcelLootDisableLootId")?.rows,
            excel_loot_rare_pool_cnt_limit: super::ConfigTable::load(dir, "ExcelLootRarePoolCntLimit")?.rows,
            excel_loot_rare_pool_item_rate: super::ConfigTable::load(dir, "ExcelLootRarePoolItemRate")?.rows,
            excel_loot_source_map: super::ConfigTable::load(dir, "ExcelLootSourceMap")?.rows,
            excel_loot_ue_source_choose_group: super::ConfigTable::load(dir, "ExcelLootUeSourceChooseGroup")?.rows,
            excel_loot_unlucky_value: super::ConfigTable::load(dir, "ExcelLootUnluckyValue")?.rows,
            excel_lottery_access_key: super::ConfigTable::load(dir, "ExcelLotteryAccessKey")?.rows,
            excel_lottery_pool: super::ConfigTable::load(dir, "ExcelLotteryPool")?.rows,
            excel_mail_template: super::ConfigTable::load(dir, "ExcelMailTemplate")?.rows,
            excel_map_detail: super::ConfigTable::load(dir, "ExcelMapDetail")?.rows,
            excel_map_exp: super::ConfigTable::load(dir, "ExcelMapExp")?.rows,
            excel_map_select: super::ConfigTable::load(dir, "ExcelMapSelect")?.rows,
            excel_map_select_model_feature: super::ConfigTable::load(dir, "ExcelMapSelectModelFeature")?.rows,
            excel_market_info: super::ConfigTable::load(dir, "ExcelMarketInfo")?.rows,
            excel_market_poor_man: super::ConfigTable::load(dir, "ExcelMarketPoorMan")?.rows,
            excel_market_popup_video: super::ConfigTable::load(dir, "ExcelMarketPopupVideo")?.rows,
            excel_market_product_item: super::ConfigTable::load(dir, "ExcelMarketProductItem")?.rows,
            excel_market_refresh_pool: super::ConfigTable::load(dir, "ExcelMarketRefreshPool")?.rows,
            excel_market_special_cost: super::ConfigTable::load(dir, "ExcelMarketSpecialCost")?.rows,
            excel_market_tab_info: super::ConfigTable::load(dir, "ExcelMarketTabInfo")?.rows,
            excel_mithril_force_item: super::ConfigTable::load(dir, "ExcelMithrilForceItem")?.rows,
            excel_mithril_force_prestige: super::ConfigTable::load(dir, "ExcelMithrilForcePrestige")?.rows,
            excel_mithril_force_settlement: super::ConfigTable::load(dir, "ExcelMithrilForceSettlement")?.rows,
            excel_mithril_force_settlement_factor: super::ConfigTable::load(dir, "ExcelMithrilForceSettlementFactor")?.rows,
            excel_module_unlock_circular: super::ConfigTable::load(dir, "ExcelModuleUnlockCircular")?.rows,
            excel_module_unlock_type: super::ConfigTable::load(dir, "ExcelModuleUnlockType")?.rows,
            excel_quest_group_list: super::ConfigTable::load(dir, "ExcelQuestGroupList")?.rows,
            excel_quest_list: super::ConfigTable::load(dir, "ExcelQuestList")?.rows,
            excel_quest_progress_condition: super::ConfigTable::load(dir, "ExcelQuestProgressCondition")?.rows,
            excel_quest_progress_type: super::ConfigTable::load(dir, "ExcelQuestProgressType")?.rows,
            excel_quest_trigger_event_type: super::ConfigTable::load(dir, "ExcelQuestTriggerEventType")?.rows,
            excel_rank_period_reward_pool: super::ConfigTable::load(dir, "ExcelRankPeriodRewardPool")?.rows,
            excel_rank_report_setting: super::ConfigTable::load(dir, "ExcelRankReportSetting")?.rows,
            excel_rank_rule: super::ConfigTable::load(dir, "ExcelRankRule")?.rows,
            excel_role_default_equipment: super::ConfigTable::load(dir, "ExcelRoleDefaultEquipment")?.rows,
            excel_role_face_pinching_limit: super::ConfigTable::load(dir, "ExcelRoleFacePinchingLimit")?.rows,
            excel_role_head_icon: super::ConfigTable::load(dir, "ExcelRoleHeadIcon")?.rows,
            excel_role_level_exp: super::ConfigTable::load(dir, "ExcelRoleLevelExp")?.rows,
            excel_role_level_upgrade: super::ConfigTable::load(dir, "ExcelRoleLevelUpgrade")?.rows,
            excel_role_skill: super::ConfigTable::load(dir, "ExcelRoleSkill")?.rows,
            excel_role_skin_type: super::ConfigTable::load(dir, "ExcelRoleSkinType")?.rows,
            excel_role_type: super::ConfigTable::load(dir, "ExcelRoleType")?.rows,
            excel_season_rule: super::ConfigTable::load(dir, "ExcelSeasonRule")?.rows,
            excel_skill_calculate: super::ConfigTable::load(dir, "ExcelSkillCalculate")?.rows,
            excel_skill_num_exchange: super::ConfigTable::load(dir, "ExcelSkillNumExchange")?.rows,
            excel_tiers: super::ConfigTable::load(dir, "ExcelTiers")?.rows,
            excel_tiers_kill_plyaer_rule: super::ConfigTable::load(dir, "ExcelTiersKillPlyaerRule")?.rows,
            excel_tiers_show_group: super::ConfigTable::load(dir, "ExcelTiersShowGroup")?.rows,
            excel_trade_commodity: super::ConfigTable::load(dir, "ExcelTradeCommodity")?.rows,
            excel_trade_item_require: super::ConfigTable::load(dir, "ExcelTradeItemRequire")?.rows,
            excel_trade_market: super::ConfigTable::load(dir, "ExcelTradeMarket")?.rows,
            excel_trade_standard_price: super::ConfigTable::load(dir, "ExcelTradeStandardPrice")?.rows,
            excel_trade_valid_price: super::ConfigTable::load(dir, "ExcelTradeValidPrice")?.rows,
            excel_trade_valid_time: super::ConfigTable::load(dir, "ExcelTradeValidTime")?.rows,
            uesource_ability_set: super::ConfigTable::load(dir, "UESourceAbilitySet")?.rows,
            uesource_item_type: super::ConfigTable::load(dir, "UESourceItemType")?.rows,
            ue_source_actor_skin: super::ConfigTable::load(dir, "UeSourceActorSkin")?.rows,
            ue_source_character: super::ConfigTable::load(dir, "UeSourceCharacter")?.rows,
            ue_source_equipment: super::ConfigTable::load(dir, "UeSourceEquipment")?.rows,
            ue_source_equipment_sub_type: super::ConfigTable::load(dir, "UeSourceEquipmentSubType")?.rows,
            excel_origin_const_config: super::ConfigTable::load(dir, "excel_origin_const_config")?.rows,
        })
    }
}
