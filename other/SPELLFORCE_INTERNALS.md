## 1. PAK Archive & Virtual File System (VFS)

### 1.1. SpellForce 1 — `MASSIVE PAKFILE V 4.0`
The Phenomic VFS mounts archives sequentially (`sf0.pak` through `sf35.pak` up to `sfXX.pak`). Archives mounted later override identically named files in earlier archives. Loose files on disk take top priority.

#### Binary Layout Overview
```text
+-----------------------------------------------------------------------+
| Offset 00..28  | Magic & Version ('MASSIVE PAKFILE V 4.0\r\n\0')      |
+-----------------------------------------------------------------------+
| Offset 28..72  | Static Phenomic Engine Header Template (44 bytes)    |
+-----------------------------------------------------------------------+
| Offset 72..76  | Custom IEEE 802.3 CRC32 Checksum (Little-Endian)      |
+-----------------------------------------------------------------------+
| Offset 76..80  | File Count (uint32)                                  |
+-----------------------------------------------------------------------+
| Offset 80..84  | Root Search Index (uint32, typically meshes.txt)     |
+-----------------------------------------------------------------------+
| Offset 84..88  | Data Section Start Offset (uint32)                   |
+-----------------------------------------------------------------------+
| Offset 88..92  | Total Archive Size (uint32, 4096-byte aligned)       |
+-----------------------------------------------------------------------+
| Offset 92..EOF | File Table -> String Table -> Data Payload           |
+-----------------------------------------------------------------------+
```

#### Core Engine Structures
```rust
// 44-byte Phenomic Runtime Header Template (Offsets 28..72)
const PHENOMIC_HEADER_TEMPLATE: [u8; 44] = [
    0x00, 0x00, 0x00, 0x00, 0xb0, 0xff, 0x12, 0x00, 0x08, 0x6f, 0x40, 0x00, 0x38, 0xc1, 0x40, 0x00,
    0xff, 0xff, 0xff, 0xff, 0x40, 0x28, 0x32, 0x00, 0x52, 0x48, 0x40, 0x00, 0x1f, 0x00, 0x00, 0x00,
    0xda, 0x31, 0x40, 0x00, 0x1f, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff,
];

// File Table Entry (16 bytes per file)
#[repr(C, packed)]
struct SF1FileEntry {
    file_size: u32,
    data_offset: u32,       // Relative to data_start_offset
    name_offset: u32,       // Low 24 bits: offset in String Table
    dir_offset: u32,        // Low 24 bits: directory string offset (0 or 0x00FFFFFF = root)
}
```

#### The In-Engine 16-Bit K&R Path Hash (`FUN_004a4180`)
Inside `SpellForce.exe`, file lookup is accelerated by prepending a 2-byte hash to each filename in the String Table:

$$\text{hash} = \left( \sum (\text{hash} \times 31 + \text{char}) \right) \mathbin{\text{AND}} \text{0xFFFF}$$

* **Sorting Comparator:** Binary search inside the archive requires records to be sorted strictly by:
  `hash_high_byte -> hash_low_byte -> reversed_path_string`.
* **String Table Alignment Bug (`D3DERR_INVALIDCALL`):**
  DirectX 8/9 vertex and index buffer upload routines expect file data offsets to be aligned to **4-byte DWORD boundaries**. If the String Table ends on an unaligned byte, the graphics driver crashes upon reading meshes.
  * **Fix:** The String Table must be padded with zeroes:

    $$\text{pad} = (4 - (\text{str\_table\_len} \pmod 4)) \pmod 4$$

#### Custom CRC32 Algorithm
Standard IEEE 802.3 polynomial (`0xEDB88320`), but **without final bitwise XOR inversion**:
```rust
fn calculate_sf1_crc(data: &[u8], prev_crc: u32) -> u32 {
    let mut crc = prev_crc;
    for &b in data {
        crc = (crc >> 8) ^ CRC32_TABLE[((crc ^ (b as u32)) & 0xFF) as usize];
    }
    crc
}
```

---

### 1.2. SpellForce 2 — `PAK\x01`
* **Signature:** `b"PAK\x01"` (4 bytes).
* **Header:**
  * `0x04`: Directory Offset (`u32`)
  * `0x08`: Uncompressed Directory Size (`u32`)
  * `0x0C`: Compressed Directory Size (`u32`)
* **Directory Structure:** Zlib-compressed stream containing:
  * `num_files: i32`
  * Repeated `num_files` times:
    * `name_length: i32`
    * `filename_bytes: [u8; name_length]` (Windows-1252/1251, lowercase, backslashes)
    * `offset: u32`
    * `next_offset: u32` (`size = next_offset - offset`)

---

## 2. CFF Database Container Engine

### 2.1. Container Signatures & Versions
1. **SpellForce 1 Classic (2003, *The Order of Dawn*):**
   * Signature: `0xDD72C502` (`b"\x02\xc5\x72\xdd"`).
2. **SpellForce 1 Platinum Edition (1.52–1.54):**
   * Signature: `0xDD72DD12` (`b"\x12\xdd\x72\xdd"`).
   * Verified by header check: `h2 == 2 && h3 == 2 && h4 == 1 && h5 == 0`.
3. **SpellForce 2 (v3.0):**
   * Signature: `0xDD72DD12` (`b"\x12\xdd\x72\xdd"`).
   * Version: `0x00000003` at offset 4, followed by 12 reserved zero bytes.

---

### 2.2. The 8,192 Chunk Limit (`CChunkFile` Object Layout)
Inside `SpellForce.exe`, `CGameDatabase::Load` allocates:

$$\text{Allocation Size} = \mathbf{\text{0x2803C}}\text{ bytes (163,900 bytes)}.$$

#### Architectural Deconstruction
* Base structure / VFS handles: `0x38` bytes.
* Chunk Table of Contents (TOC) array: `0x28000` bytes (163,840 bytes).
* Stride per TOC entry: **20 bytes**.

$$\text{Max Chunks} = \frac{163,840}{20} = \mathbf{8,192\text{ chunks}}.$$

* Offset `0x28038`: Active chunk count (`uint32`).
* Search algorithm (`FUN_005a4670`): Scans the 20-byte TOC array matching:
  `chunk_id == target_id && occurrence == 0 && c_type == target_type`.

---

### 2.3. Chunk Headers Specification

#### SpellForce 2 Chunk Header (16 bytes)
```rust
#[repr(C, packed)]
pub struct SF2ChunkHeader {
    pub chunk_id: u32,     // e.g. 0x234E (Abilities)
    pub schema_flag: u16,  // 0x0001
    pub comp_size: u32,    // Zlib payload size
    pub element_type: u16, // 1 = Raw Bytes/Structs/Strings, 2 = UTF-16/u16 IDs, 4 = u32/Float
    pub uncomp_size: u32,  // Decompressed payload size
}
```

#### SpellForce 1 Chunk Header (12 bytes base + 4 bytes optional)
```rust
#[repr(C, packed)]
pub struct SF1ChunkHeader {
    pub chunk_id: u16,     // e.g. 0x07DC (2012)
    pub occurrence: i16,   // Usually 0
    pub comp_flag: i16,    // 0 = Uncompressed, 1 = Zlib compressed
    pub comp_size: i32,    // Size on disk
    pub c_type: i16,       // Standard = 1, EXCEPT 0x0800 where c_type == 3!
    // If comp_flag != 0:
    // pub uncomp_size: i32
}
```

---

### 2.4. Reversed SF1 Chunk Registry & Record Strides
Disassembled from `CGameDatabase::Load` (`FUN_0091a930`):

| Chunk ID (Hex) | Chunk ID (Dec) | Required `c_type` | Record Stride | Data Layout & In-Engine Function |
| :---: | :---: | :---: | :---: | :--- |
| **`0x07DC`** | 2012 | 1 | **69 bytes** | `u16 id, u8 flag, char[64] mesh, u16 extra` (`2dGfxItems`) |
| **`0x07E1`** | 2017 | 1 | **6 bytes** | `u16 key -> vector<u16, u8>` (`SpellMultiMap`) |
| **`0x07E2`** | 2018 | 1 | **4 bytes** | `u16 spell_id, u16 scroll_id` (`SpellsBiMap`) |
| **`0x07F7`** | 2039 | 1 | **4 bytes** | `u16 key, u16 val` (`TextDialogueMap`) |
| **`0x07FC`** | 2044 | 1 | **3 bytes** | `u8 key, u16 val` (`TypeCategoryMap`) |
| **`0x07FF`** | 2047 | 1 | **5 bytes** | `u16 key -> vector<u16, u8>` (`EntityLinkMap`) |
| **`0x0800`** | 2048 | **3** ⚠️ | **15 bytes** | **ComplexProperties**: `u8 key, u32 f1, u32 f2, u16 f3, u32 f4` |
| **`0x0801`** | 2049 | 1 | **2 bytes** | Flat array of `u16` words (`WordValuesArray`) |
| **`0x080A`** | 2058 | 1 | **4 bytes** | `u16 str_id, u16 text_id` (`LocalizedStringIds`) |
| **`0x080B`** | 2059 | 1 | **6 bytes** | `u32 id, u16 param` (`AudioSpeechParams`) |
| **`0x080E`** | 2062 | 1 | **9 bytes** | `u16 compound_key, u32 f1, u16 f2, u8 f3` |
| **`0x0818`** | 2072 | 1 | **4 bytes** | `u8/u16 key, u16 val` (`SystemLookupMap` / Cascade terminator) |

---

## 3. Localized Strings & The `Fixed566` Engine

### 3.1. Block Layout (566 bytes per record)
* `0x00..0x04` (`4 bytes`): `str_id` (32-bit unsigned integer).
* `0x04..0x36` (`50 bytes`): Metadata / Reserved flags.
* `0x36..0x236` (`512 bytes`): Null-terminated string buffer.

### 3.2. Mathematical Bitmask Architecture of `str_id`

$$\text{str\_id} = (\text{CampaignID} \ll 24) \mid (\text{LanguageID} \ll 16) \mid \text{BaseID}$$

```rust
let base_id = (str_id & 0xFFFF) as u16;          // Bits 0..15: String index
let language_id = ((str_id >> 16) & 0xFF) as u8; // Bits 16..23: Language Slot
let campaign_id = (str_id >> 24) as u8;          // Bits 24..31: Campaign / Addon
```

#### Language Slots Table
| Slot | Tag | Language | Primary Encoding |
| :---: | :---: | :--- | :--- |
| **0** | `DE` | German | Windows-1252 |
| **1** | `EN` | English | Windows-1252 |
| **2** | `FR` | French | Windows-1252 |
| **3** | `ES` | Spanish | Windows-1252 |
| **4** | `IT` | Italian | Windows-1252 |
| **5+** | `RU` / `AR` / `PL` | Custom / Community | Windows-1251 / Custom |

#### Campaign IDs
* `0`: *The Order of Dawn* (Base Game)
* `1`: *The Breath of Winter* (Addon 1) — previously misidentified as `EXT` due to missing `& 0xFF` mask (`str_id >> 16 == 256`).
* `2`: *Shadow of the Phoenix* (Addon 2).

### 3.3. Encoding Collision Prevention
* **The Bug:** Characters in range `0xC0..=0xFF` represent Cyrillic in `Windows-1251`, but Western European accented letters and umlauts (`ä, ö, ü, ß, é, è, ê`) in `Windows-1252`.
* **Resolution Rule:**
  * Slots `0..=4` must strictly be decoded via **Windows-1252**.
  * Slot `5` (or user text containing Cyrillic characters `\u{0400}..=\u{04FF}`) is encoded via **Windows-1251**.

---

## 4. Dual Engine Loaders Architecture

Inside `SpellForce.exe`, `GameData.cff` is loaded by two separate subsystems:

```text
                           data\GameData.cff
                                  │
         ┌────────────────────────┴────────────────────────┐
         ▼                                                 ▼
   FUN_0091a930                                      FUN_00a6eab0
   Master Gameplay Database                          Sound & Speech Engine
   ├── 2dGfxItems (0x07DC)                           ├── Executes 'sound/speech/langid.lua'
   ├── SpellsBiMap (0x07E2)                          ├── Reads 'LangId' & 'ClientLanguage'
   ├── LocalizedStringIds (0x080A)                   └── Mounts localized speech & audio banks
   └── 15-step sequential cascade
```

---

## 5. Visual Asset Linkage Pipeline

### 5.1. Item Presentation (`2dGfxItems` 0x07DC)
Each item connects to graphic assets through its 69-byte descriptor:
```text
Item Record (ItemID)
     │
     └── 2dGfxItems [ItemID, Flag]
              │
              ├── Flag 1: Inventory Scroll / Item Icon (ui_item_...msh -> .dds)
              └── Flag 2: Spellbook / Action Bar Icon (ui_spell_...msh -> .dds)
```

### 5.2. Spell Linking (`SpellsBiMap` 0x07E2)
Spell casting logic is strictly decoupled from inventory scrolls:

$$\text{Spell ID (Combat Spell Entity)} \qquad \underset{\text{0x07E2}}{\overset{\text{BiMap}}{\longleftrightarrow}} \qquad \text{Scroll ID (Inventory Item Entity)}$$

---

## 7. SpellForce 2 Engine Architecture: CFF v3.0 & GDS Luabind Bridge

### 7.1. Database Dispatcher (`FUN_00718300`)
Inside `SpellForce2.exe`, database initialization allocates:

$$\text{Allocation Size} = \mathbf{\text{0x28038}}\text{ bytes (163,896 bytes)}.$$

* Fixed TOC capacity: **8,192 Chunks** (stride: 20 bytes).
* Iterates sequentially across up to **4 campaigns/addons** (`base\data\Gamedata.cff`, `addon1\data\Gamedata.cff`).
* Combines CFF binary tables with XML configuration files (`GdBaseValues.xml`, `projectileStartPositions.xml`).

### 7.2. Reversed SF2 Chunk Registry & `element_type`
Disassembled from `FUN_00718300` and chunk loader functions (`FUN_008eafe0`):

| Chunk ID (Hex) | Chunk ID (Dec) | Element Type | In-Engine Name | Structure & Purpose |
| :---: | :---: | :---: | :--- | :--- |
| **`0x2329`** | 9001 | 4 (DWORD) | `UnitAttributes` | 280-byte unit combat stats and parameters |
| **`0x232C`** | 9004 | 1 (Byte) | `CategoryTypes` | 44-byte type categories (`u8` index) |
| **`0x232F`** | 9007 | 2 (UTF-16) | `LocalizedNames` | 268-byte localized string mapping table |
| **`0x2330`** | 9008 | 4 (DWORD) | `ItemProperties` | **404-byte Item Record** (embeds `0x232F` base) |
| **`0x2335`** | 9013 | 1 (Byte) | `VisualMeshes` | **Direct successor to `0x07DC`** (item/unit mesh bindings) |
| **`0x2341`** | 9025 | 1 (Byte) | `StringLinks` | 244-byte dynamic string linkages |
| **`0x2345`** | 9029 | 1 (Byte) | `SpellParameters` | 116-byte spell effect parameter definitions |
| **`0x2349`** | 9033 | 1 (Byte) | `MultiplierTriplets` | 16-byte fixed vectors (`ID + 3 * u32`) |
| **`0x234E`** | 9038 | 1 (Byte) | `Abilities` | Ability records (4 `std::string` fields: mesh, icon, tag, sound) |

#### Item Record Structure Layout (Chunk `0x2330`, 404 Bytes)
Disassembled from `FUN_007121c0`:
* `0x000..0x10B` (268 bytes): Embedded Base Record (`0x232F` localized descriptor, `FUN_0070f1c0`).
* `0x10C` (4 bytes): `u32` Item Type / Primary ID.
* `0x110` (4 bytes): `u32` Sell / Gold Value.
* `0x114` (2 bytes): `u16` Level Requirement.
* `0x116` (2 bytes): `u16` Stat / Skill Requirement.
* `0x118..0x133` (28 bytes): `std::string` 3D Mesh & Icon Asset Identifier (links to `0x2335`).
* `0x134..0x143` (16 bytes): Sub-struct 1 — Combat Modifiers / Damage Range (`min` / `max`).
* `0x144..0x153` (16 bytes): Sub-struct 2 — Armor Class / Elemental Resistances.
* `0x154..0x163` (16 bytes): Sub-struct 3 — Attack Speed / Range Parameters.
* `0x164..0x173` (16 bytes): Sub-struct 4 — Item Affixes & Suffix Properties.
* `0x174..0x183` (16 bytes): Sub-struct 5 — Attribute Bonuses (Strength, Agility, Intelligence).
* `0x184..0x193` (16 bytes): Sub-struct 6 — Equipment Slot Bitmask (`0x184 + 16 = 0x194` / 404 bytes total).

### 7.3. Luabind GDS Runtime Bridge (`FUN_009e5620`)
SpellForce 2 binds C++ engine actions to the Lua 5.1 GDS runtime using **Luabind**:
* `CScriptActionFigureCastSpell` $\leftrightarrow$ `FigureCastSpell`
* `CScriptActionFigureChangePlayer` $\leftrightarrow$ `FigurePlayerTransfer` / `FigureChangePlayer`
* `CScriptActionFigureKill` $\leftrightarrow$ `FigureKill`
* `CScriptActionFigureVanish` $\leftrightarrow$ `FigureVanish`
* `CScriptActionFigureStopJob` $\leftrightarrow$ `FigureStopJob`
* `CScriptActionHoldPosition` $\leftrightarrow$ `FigureHoldPosition`
* `CScriptActionCutSceneBegin` $\leftrightarrow$ `CutsceneBegin`
* `CScriptActionCutSceneEnd` $\leftrightarrow$ `CutsceneEnd`
* `CScriptActionCutSceneSay` $\leftrightarrow$ `CutsceneSay`
* `CScriptActionCutSceneOutCry` $\leftrightarrow$ `FigureOutcry`
* `CScriptActionRequestDialogBegin` $\leftrightarrow$ `DialogBegin`
* `CScriptActionPlayerTakeItem` $\leftrightarrow$ `AvatarItemMiscTake`
* `CScriptActionPlayerAddMoney` $\leftrightarrow$ `AvatarGoldGive`
* `CScriptActionPlaceObject` $\leftrightarrow$ `PlaceObject`
* `CScriptActionChangeObject` $\leftrightarrow$ `ChangeObject`
* `CScriptActionValueRandom` $\leftrightarrow$ `AvatarValueRandomize`
* Custom navigation and waypoint members: `AddGotoPoint`, `AddGotoEntity`, and `Scout`.
```
