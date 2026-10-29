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



# SpellForce 2: Demons of the Past — Post-Processing Subsystem & Engine Internals Reference

Technical reference manual documenting the native rendering pipeline, parameter bindings, vertex layouts, format lookup tables, and binary structure of the post-processing subsystem in *SpellForce 2: Demons of the Past* (Phenomic Game Development / Nordic Games).

---

## Table of Contents
1.  [Overview & Authorship](#1-overview--authorship)
2.  [Engine Subsystem Architecture](#2-engine-subsystem-architecture)
3.  [Native Post-Processing Pipeline](#3-native-post-processing-pipeline)
4.  [Class Hierarchy & VTable Architecture](#4-class-hierarchy--vtable-architecture)
5.  [Shader Compilation & Parameter Reflection](#5-shader-compilation--parameter-reflection)
6.  [Fullscreen Quad & Vertex Declaration](#6-fullscreen-quad--vertex-declaration)
7.  [Engine Parameter Registers (`Offset` Semantics)](#7-engine-parameter-registers-offset-semantics)
8.  [Native Shader Implementations & Algorithms](#8-native-shader-implementations--algorithms)
9.  [The Texture Format Subsystem (Table `0x00c5e3a8`)](#9-the-texture-format-subsystem-table-0x00c5e3a8)
10. [Binary Patch: Unlocking FP16 Render Targets](#10-binary-patch-unlocking-fp16-render-targets)
11. [Format Selection Analysis: `A16B16G16R16F` vs `R32F` vs `A32B32G32R32F`](#11-format-selection-analysis-a16b16g16r16f-vs-r32f-vs-a32b32g32r32f)

---

## 1. Overview & Authorship

The native DirectX 9 (Shader Model 3.0) post-processing pipeline of *SpellForce 2: Demons of the Past* was authored between 2012 and 2014 by **Marc 'Urban' Muenchow** (Nordic Games):
- `pp_glow.fx`, `pp_fxaa.fx`, `pp_sharpen.fx`, `pp_blur.fx`, `pp_noise.fx` (Initial Release: June 24, 2012)
- `pp_color_grading.fx` (Initial Release: August 31, 2012)
- Minor revisions and enhancements (e.g., contrast/vibrance adjustments in `pp_noise.fx`) continued through April 6, 2014.

All shaders operate in standard display-referred gamma space and target a 32-bit ARGB surface (`D3DFMT_A8R8G8B8`) by default.

---

## 2. Engine Subsystem Architecture

Disassembly of `SpellForce2.exe` via Ghidra reveals that screen-space image processing is strictly separated from 3D world visual effects:

```
                  ┌────────────────────────────────────────────────────────┐
                  │                    SpellForce2.exe                     │
                  └───────────────────────────┬────────────────────────────┘
                                              │
                    ┌─────────────────────────┴─────────────────────────┐
                    ▼                                                   ▼
┌───────────────────────────────────────┐   ┌───────────────────────────────────────┐
│     VFX & Track Subsystem (0x00904d10)│   │   Post-Processing Display Framework   │
│  - CFxTrack / CFxTrackFloat RTTI      │   │   - CDspBasePostProcessingEffect...   │
│  - Particle Graphs (.fxe emitters)    │   │   - Fullscreen quad management        │
│  - Animation keyframes & curves       │   │   - Native .fx pipeline execution     │
└───────────────────────────────────────┘   └───────────────────┬───────────────────┘
                                                                │
                                            ┌───────────────────┴───────────────────┐
                                            ▼                                       ▼
                         ┌─────────────────────────────────────┐ ┌─────────────────────────────────────┐
                         │   Direct3D 9 Device (DAT_00d0bd2c)  │ │   Texture Manager (DAT_00d0c1f0)   │
                         │   - Pointer at offset +0x7c         │ │   - Format table at 0x00c5e3a8      │
                         │   - Native IDirect3DDevice9 vtable  │ │   - Render Target allocation        │
                         └─────────────────────────────────────┘ └─────────────────────────────────────┘
```

- **VFX & Track Subsystem (`0x00904d10`)**:
  - Deserializes and evaluates `.fxe` effect emitter files.
  - Employs RTTI types `CFxTrack` and `CFxTrackFloat` to interpolate animation curves (color fades, opacity decay, particle life cycles).
- **Post-Processing Framework (`CDspBasePostProcessingEffectBaseDX9`)**:
  - Base class governing all screen-space full-frame passes.
  - Manages render target allocation, quad vertex declarations, and runtime shader compilation.
- **Global Singletons**:
  - `DAT_00d0bd2c + 0x7c`: Primary `IDirect3DDevice9` COM interface pointer.
  - `DAT_00d0c1f0` / `DAT_00d0c45c`: Global `CTextureManagerDX9` singleton controlling texture creation and format translation.

---

## 3. Native Post-Processing Pipeline

The post-processing passes are hardcoded engine stages. Their initializers are compiled sequentially in `.text` memory:

| Initializer Address | Shader File | Pass Functionality |
| :---: | :---: | :--- |
| `0x004996a0` | `pp_color_grading.fx` | 3D LUT sampling ($16\times16\times16$) and LUT cross-fading |
| `0x00499a20` | `pp_noise.fx` | Per-pixel vibrance, S-curve contrast, animated film grain |
| `0x00499e60` | `pp_sharpen.fx` | 3-tap unsharp masking with luminance thresholding |
| `0x0049a1e0` | `pp_glow.fx` | Highlight masking, chromatic aberration, Screen-mode bloom blend |
| `0x0049a5c0` | `pp_blur.fx` | Separable 9-tap & 13-tap Gaussian blur, bloom desaturation |
| `0x0049a890` | `pp_fxaa.fx` | Timothy Lottes' FXAA 3.11 with green-channel luma optimization |
| *(Utility)*   | `pp_debug.fx` | Buffer inspection tool (4x zoom in top-left screen quadrant) |

### Native Execution Order
$$\text{Color Grading} \longrightarrow \text{Noise} \longrightarrow \text{Sharpen} \longrightarrow \text{Glow / Blur (Sub-pass)} \longrightarrow \text{FXAA}$$

1. **`pp_color_grading.fx`**: Evaluated first on the raw backbuffer. Applies external color grading via a 2.5D LUT strip atlas.
2. **`pp_noise.fx`**: Adds color vibrance, S-curve contrast, and animated grain to the graded image.
3. **`pp_sharpen.fx`**: Enhances edge contrast using an unsharp mask kernel.
4. **`pp_blur.fx` & `pp_glow.fx`**:
   - `pp_glow.fx` executes `technique MaskHighLights` to isolate pixels exceeding luminance threshold $0.45$.
   - `pp_blur.fx` runs horizontal and vertical separable blur passes on the isolated highlights (`Blur9H` / `Blur9V` or `Blur13H` / `Blur13V`).
   - `pp_glow.fx` executes `technique Glow` to composite the blurred highlights over the scene using inverted multiplicative (Screen) blending.
5. **`pp_fxaa.fx`**: Final display stage. Performs anti-aliasing prior to presentation.

---

## 4. Class Hierarchy & VTable Architecture

All post-processing passes derive from `CDspBasePostProcessingEffectBaseDX9`.

### Base Initialization Sequence (`FUN_004e18d0`)
```c
int __thiscall CDspBasePostProcessingEffectBaseDX9::Initialize(this, int width, int height)
{
    this->m_pDevice = *(IDirect3DDevice9**)(DAT_00d0bd2c + 0x7c);
    if (!this->m_pDevice) return 0;

    this->m_width  = width;
    this->m_height = height;

    // 1. Allocate Render Targets (FUN_004e13d0)
    if (!this->vtable[12]()) return 0; // +0x30

    // 2. Compile Shader and Reflect Parameters (FUN_004e1950)
    if (this->vtable[14]() < 0) {      // +0x38 (SUCCEEDED check: hr >= 0)
        this->vtable[13]();            // +0x34 (Rollback Render Targets)
        return 0;
    }

    // 3. Create Vertex Declaration and Quad Buffers (FUN_004e0eb0)
    if (this->vtable[16]() < 0) {      // +0x40
        this->vtable[13]();            // +0x34 (Rollback Render Targets)
        this->vtable[15]();            // +0x3c (Rollback Shader)
        return 0;
    }

    return 1;
}
```

### VTable Method Layout (`PTR_FUN_00b1c2fc`)

| Offset | Index | Function | Role |
| :---: | :---: | :--- | :--- |
| `+0x00` | `[0]` | `FUN_004996a0` | Pass `Init()` method (stores shader filename) |
| `+0x04` | `[1]` | `LAB_00499470` | Scalar deleting destructor |
| `+0x30` | `[12]` | `FUN_004e13d0` | `CreateRenderTargets()` — Allocates `CTexture` surface |
| `+0x34` | `[13]` | `FUN_004e0e70` | `ReleaseRenderTargets()` — Releases surface COM handles |
| `+0x38` | `[14]` | `FUN_004e1950` | `CreateShader()` — Compiles `.fx` effect from file |
| `+0x3c` | `[15]` | `FUN_004e0e90` | `ReleaseShader()` — Releases `ID3DXEffect` interface |
| `+0x40` | `[16]` | `FUN_004e0eb0` | `CreateVertexData()` — Builds vertex declaration & quad VB |
| `+0x44` | `[17]` | `FUN_004e0fa0` | `ReleaseVertexData()` — Releases quad vertex buffers |
| `+0x48` | `[18]` | `FUN_004e14b0` | Quad geometry update / viewport setup |
| `+0x50` | `[20]` | `LAB_004995e0` | `OnParameter()` — Virtual callback for uniform parameter binding |
| `+0x54` | `[21]` | `LAB_00499650` | `OnTechnique()` — Virtual callback for technique handle binding |

---

## 5. Shader Compilation & Parameter Reflection

### Compilation Settings (`FUN_004e1950`)
The engine invokes D3DX to parse `.fx` files directly from disk:
```cpp
D3DXCreateEffectFromFileA(
    this->m_pDevice,              // IDirect3DDevice9*
    this->m_shaderPath,           // "pp_color_grading.fx", etc.
    NULL,                         // D3DXMACRO*
    NULL,                         // LPD3DXINCLUDE
    0x800,                        // Flags: D3DXFX_NOT_CLONEABLE
    NULL,                         // LPD3DXEFFECTPOOL
    &this->m_pEffect,             // LPD3DXEFFECT*
    &pCompilationErrors           // LPD3DXBUFFER*
);
```
If compilation fails, error text is extracted via `pCompilationErrors->GetBufferPointer()` and written to the engine log:
`"CDspBasePostProcessingEffectBaseDX9::CreateShader: D3DXCreateEffectFromFile FAILED! Missing shader file (%s) ?"`

### Automated Reflection Mechanism
Following compilation, the engine reflects parameters using `ID3DXEffect::GetDesc`:

1. **Parameters Loop**:
   - Queries count via `D3DXEFFECT_DESC.Parameters`.
   - Obtains parameter handles using `m_pEffect->GetParameter(NULL, i)`.
   - Retrieves name and type using `m_pEffect->GetParameterDesc(hParam, &desc)`.
   - Passes handle and name to `vtable[20]` (`+0x50`).
2. **Techniques Loop**:
   - Queries count via `D3DXEFFECT_DESC.Techniques`.
   - Obtains technique handles using `m_pEffect->GetTechnique(i)`.
   - Retrieves name using `m_pEffect->GetTechniqueDesc(hTech, &desc)`.
   - Passes handle and name to `vtable[21]` (`+0x54`).

### Variable Binding Implementation (`0x004995e0`)
String comparisons (`strncmp`) map engine variables to internal fields:
- `"SceneTexture"` $\rightarrow$ Handle stored at `this + 0x94`
- `"LUTTexture"`   $\rightarrow$ Handle stored at `this + 0x98`
- `"Offset"`       $\rightarrow$ Handle stored at `this + 0x8c`

Technique strings are mapped to fields initialized to `-1` (`0xFFFFFFFF`):
- `"ColorGrading"`   $\rightarrow$ Handle stored at `this + 0xa4`
- `"ColorLUTBlend"`  $\rightarrow$ Handle stored at `this + 0xa0`

---

## 6. Fullscreen Quad & Vertex Declaration

In `CDspBasePostProcessingEffectBaseDX9::CreateVertexData` (`FUN_004e0eb0`), the quad geometry is constructed using standard DirectX 9 FVF declarations:

### Vertex Declaration Elements (`D3DVERTEXELEMENT9`)
```cpp
const D3DVERTEXELEMENT9 g_PostProcessVertexElements[] =
{
    { 0,  0, D3DDECLTYPE_FLOAT4, D3DDECLMETHOD_DEFAULT, D3DDECLUSAGE_POSITION, 0 },
    { 0, 16, D3DDECLTYPE_FLOAT4, D3DDECLMETHOD_DEFAULT, D3DDECLUSAGE_TEXCOORD, 0 },
    D3DDECL_END()
};
```

### Vertex Buffer Specifications
- **Size**: `0x80` (128 bytes total) = 4 vertices $\times$ 32 bytes.
- **FVF Layout**:
  - `Offset 0..15`:  `float4 Position` (Homogeneous clip space: $x, y, z, w$)
  - `Offset 16..31`: `float4 TexCoord` ($u, v$, auxiliary scale parameters)
- **DirectX Call**:
  ```cpp
  m_pDevice->CreateVertexDeclaration(g_PostProcessVertexElements, &this->m_pDecl);
  m_pDevice->CreateVertexBuffer(128, D3DUSAGE_WRITEONLY, 0, D3DPOOL_DEFAULT, &this->m_pVB, NULL);
  ```

---

## 7. Engine Parameter Registers (`Offset` Semantics)

Across all post-processing shaders, the global vector `float4 Offset` is populated by the engine with specific per-pass data:

| Shader Name | `Offset.xy` | `Offset.zw` |
| :--- | :--- | :--- |
| **`pp_color_grading.fx`** | Texel size: $(\frac{1}{\text{Width}}, \frac{1}{\text{Height}})$ | `Offset.w`: Interpolation factor for `ColorLUTBlend` (`0.0` = Scene, `1.0` = LUT) |
| **`pp_noise.fx`** | Tiling frequency scale $(\text{UV} \times \text{Scale})$ | Animated temporal scroll vector: $(\text{Phase}_X, \text{Phase}_Y)$ |
| **`pp_sharpen.fx`** | Texel size: $(\frac{1}{\text{Width}}, \frac{1}{\text{Height}})$ | `Offset.z`: Sharpen strength multiplier<br>`Offset.w`: Sharpen clamping limit |
| **`pp_glow.fx`** | Texel size: $(\frac{1}{\text{Width}}, \frac{1}{\text{Height}})$ | Chromatic aberration displacement vector |
| **`pp_blur.fx`** | Texel size: $(\frac{1}{\text{Width}}, \frac{1}{\text{Height}})$ | Unused / Reserved |
| **`pp_fxaa.fx`** | Texel size: $(\frac{1}{\text{Width}}, \frac{1}{\text{Height}})$ | Unused / Reserved |
| **`pp_debug.fx`** | Texel size: $(\frac{1}{\text{Width}}, \frac{1}{\text{Height}})$ | Unused / Reserved |

---

## 8. Native Shader Implementations & Algorithms

### 8.1. Color Grading (`pp_color_grading.fx`)
Employs an uncompressed 2.5D horizontal strip atlas texture representing a 3D cube LUT:
- **Dimensions**: Hardcoded to $16 \times 16 \times 16$ slices:
  ```hlsl
  const float2 fWidth = float2(16.0f, 1.0f / 16.0f);
  const float2 fScale = float2((fWidth.x - 1.0f) * fWidth.y, 0.5f * fWidth.y);
  ```
- **Sampling Logic**: Blue channel selects adjacent 2D slices along the horizontal axis, which are bilinearly sampled and linearly interpolated via `frac(b)`.

### 8.2. Separable Blur (`pp_blur.fx`)
Implements optimized 1D Gaussian blurs leveraging bilinear hardware texture filtering:
- **`Blur9`**: 5 taps covering 9 effective samples:
  - Offsets: $1.3846153846 \times \text{Offset}$ and $3.2307692308 \times \text{Offset}$
  - Weights: Center $0.2270270270$, Inner $0.3162162162$, Outer $0.0702702703$
- **`Blur13`**: 7 taps covering 13 effective samples:
  - Offsets: $1.4117647058 \times \text{Offset}$, $3.2941176468 \times \text{Offset}$, and $5.1764705871 \times \text{Offset}$
  - Weights: Center $0.1964825501$, Inner $0.2969069646$, Mid $0.0944703978$, Outer $0.0103813624$
- **Orientation**: Pass uniforms `float2(1.0, 0.0)` for horizontal and `float2(0.0, 1.0)` for vertical execution.

### 8.3. Bloom & Chromatic Aberration (`pp_glow.fx`)
- **Highlight Masking (`PS_MaskHighLights`)**:
  - Uses standard Rec.601 luma coefficients (`float3(0.3, 0.59, 0.11)`).
  - Hardcoded thresholding:
    ```hlsl
    cOut *= max(dot(cOut.rgb, float3(0.3f, 0.59f, 0.11f)) - 0.45f, 0.0f) * 2.25f;
    ```
- **Chromatic Aberration & Composition (`PS_Glow`)**:
  - Offsets the red and blue bloom taps using screen-space chromatic vector scaled by bloom luma:
    ```hlsl
    float lum = dot(cGlow.rgb, float3(0.3f, 0.59f, 0.11f));
    float2 Chroma = texCoord.zw * lum; // texCoord.zw = Offset.zw * 2.50f
    cGlow.r = _TexTop(GlowImage, texCoord.xy + Chroma.xy).r;
    cGlow.b = _TexTop(GlowImage, texCoord.xy - Chroma.xy).b;
    ```
  - **Screen Blending**: Avoids additive overflow in 8-bit SDR by using inverted multiplicative blending:
    $$\text{Result} = C_{\text{Scene}} + C_{\text{Glow}} \times (1.0 - C_{\text{Scene}})$$

### 8.4. Noise & Film Grain (`pp_noise.fx`)
- **Vibrance**: Non-linear saturation boost targeting muted tones:
  ```hlsl
  float colsat = max(cOut.r, max(cOut.g, cOut.b)) - min(cOut.r, min(cOut.g, cOut.b));
  cOut.rgb = lerp(lum, cOut.rgb, 1.0f + (0.10f * (1.0f - colsat)));
  ```
- **Contrast**: Midtone S-curve using `smoothstep(0.0, 1.0, lum)`.
- **Grain**: Multiplicative noise sampling `NoiseImage` using point sampling with wrap addressing, weighted towards shadows:
  $$\text{NoiseWeight} = (1.0 - \text{Luma})^{7.0} \times 0.15$$

### 8.5. Unsharp Masking (`pp_sharpen.fx`)
- Calculates a 3-tap diagonal blur kernel:
  ```hlsl
  float2 TexOffset = 1.3333333330f * Offset.xy;
  float3 cSharp = cOut * 0.2941176470f +
                  _TexTop(SceneImage, uv + TexOffset).rgb * 0.3529411764f +
                  _TexTop(SceneImage, uv - TexOffset).rgb * 0.3529411764f;
  ```
- Applies high-frequency difference scaled by `Offset.z` (strength) and clamped by `Offset.w` (limit).

### 8.6. FXAA 3.11 (`pp_fxaa.fx`)
- Urban's customized port of Timothy Lottes' Fast Approximate Anti-Aliasing 3.11.
- **Fast Luma**: Evaluates edge contrast using the green channel (`rgba.y`) as a zero-cost luminance proxy (`#define FxaaLuma(rgba) rgba.y`).
- Quality preset steps: 7 search samples along the detected edge span (`P0` through `P6`).

---

## 9. The Texture Format Subsystem (Table `0x00c5e3a8`)

The game maps internal format identifiers to native DirectX 9 `D3DFORMAT` enums via a statically allocated table at `0x00c5e3a8`.

### Memory Structure
Each entry spans 12 bytes (`0x0C`):
- `+0x00`: `uint32_t EngineFormatID`
- `+0x04`: `uint32_t D3DFormat` (`D3DFORMAT` or FOURCC)
- `+0x08`: `uint16_t BitsPerPixel`
- `+0x0A`: `uint16_t Flags`

The table terminates with sentinel value `-1` (`0xFFFFFFFF`).

### Query Functions
1. **`FUN_0047b130(int engineFormatId)`**: Direct lookup table search returning the associated `D3DFORMAT`.
2. **`FUN_0047b170(int d3dFormat)`**: Reverse search returning the internal `engineFormatId`.
3. **`FUN_0047b1e0(int engineFormatId)`**: Returns pitch/bits-per-pixel attributes stored at `+0x08`.

### Complete Table Data Dump (`0x00c5e3a8` - `0x00c5e588`)

```text
Address     Engine ID  D3DFORMAT (Hex / Dec)   BPP   DirectX Format Equivalent
──────────────────────────────────────────────────────────────────────────────
0x00c5e3a8  0x00000016 0x00000014 (20)        0x18  D3DFMT_R8G8B8
0x00c5e3b4  0x00000001 0x00000015 (21)        0x20  D3DFMT_A8R8G8B8 (Default RT)
0x00c5e3c0  0x00000002 0x00000016 (22)        0x20  D3DFMT_X8R8G8B8 (BackBuffer)
0x00c5e3cc  0x00000003 0x00000017 (23)        0x10  D3DFMT_R5G6B5
0x00c5e3d8  0x0000000C 0x00000032 (50)        0x08  D3DFMT_L8
0x00c5e3e4  0x0000000D 0x00000029 (41)        0x08  D3DFMT_P8
0x00c5e3f0  0x0000000E 0x0000001C (28)        0x08  D3DFMT_A8
0x00c5e3fc  0x0000000F 0x00000034 (52)        0x08  D3DFMT_A4L4
0x00c5e408  0x00000004 0x0000003C (60)        0x10  D3DFMT_V8U8
0x00c5e414  0x00000007 0x31545844 ('DXT1')    0x08  FOURCC DXT1
0x00c5e420  0x00000008 0x32545844 ('DXT2')    0x08  FOURCC DXT2
0x00c5e42c  0x00000009 0x33545844 ('DXT3')    0x08  FOURCC DXT3
0x00c5e438  0x0000000A 0x34545844 ('DXT4')    0x08  FOURCC DXT4
0x00c5e444  0x0000000B 0x35545844 ('DXT5')    0x08  FOURCC DXT5
0x00c5e450  0x00000012 0x00000018 (24)        0x10  D3DFMT_X1R5G5B5
0x00c5e45c  0x00000013 0x00000019 (25)        0x10  D3DFMT_A1R5G5B5
0x00c5e468  0x00000014 0x0000001A (26)        0x10  D3DFMT_A4R4G4B4
0x00c5e474  0x00000015 0x0000001E (30)        0x10  D3DFMT_A8R3G3B2
0x00c5e480  0x00000017 0x0000001F (31)        0x20  D3DFMT_X4R4G4B4
0x00c5e48c  0x00000018 0x00000020 (32)        0x20  D3DFMT_A8B8G8R8
0x00c5e498  0x00000019 0x00000021 (33)        0x20  D3DFMT_X8B8G8R8
0x00c5e4a4  0x0000001A 0x00000022 (34)        0x20  D3DFMT_G16R16
0x00c5e4b0  0x0000001B 0x00000023 (35)        0x20  D3DFMT_A2R10G10B10
0x00c5e4bc  0x0000001C 0x00000024 (36)        0x40  D3DFMT_A16B16G16R16
0x00c5e4c8  0x00000010 0x00000051 (81)        0x10  D3DFMT_L16
0x00c5e4d4  0x00000011 0x00000033 (51)        0x10  D3DFMT_A8L8
0x00c5e4e0  0x00000005 0x0000003D (61)        0x10  D3DFMT_L6V5U5
0x00c5e4ec  0x00000006 0x0000003E (62)        0x20  D3DFMT_X8L8V8U8
0x00c5e4f8  0x00000021 0x00000050 (80)        0x10  D3DFMT_Q8W8V8U8
0x00c5e504  0x00000022 0x0000004D (77)        0x20  D3DFMT_D24S8 (Depth/Stencil)
0x00c5e510  0x00000023 0x0000004B (75)        0x20  D3DFMT_D16 (Depth)
0x00c5e51c  0x00000024 0x0000004F (79)        0x20  D3DFMT_D24X8 (Depth)
0x00c5e528  0x00000025 0x00000047 (71)        0x20  D3DFMT_D32 (Depth)
0x00c5e534  0x0000001D 0x00000072 (114)       0x20  D3DFMT_R32F (Float 32)
0x00c5e540  0x0000001E 0x00000070 (112)       0x20  D3DFMT_G16R16F (Float 16x2)
0x00c5e54c  0x0000001F 0x00000071 (113)       0x40  D3DFMT_A16B16G16R16F (Float 16x4)
0x00c5e558  0x00000020 0x00000074 (116)       0x80  D3DFMT_A32B32G32R32F (Float 32x4)
0x00c5e564  0x00000026 0x00000073 (115)       0x40  D3DFMT_G32R32F (Float 32x2)
0x00c5e570  0x00000027 0x0000006F (111)       0x10  D3DFMT_R16F (Float 16)
0x00c5e57c  0x00000028 0x4C4C554E ('NULL')    0x00  FOURCC 'NULL' (NVIDIA Dummy RT)
0x00c5e588  0xFFFFFFFF —                       —     End of Table Marker (-1)
```

---

## 10. Binary Patch: Unlocking FP16 Render Targets

### Investigation
In `CDspBasePostProcessingEffectBaseDX9::Reset` (`FUN_004e1760`), the engine initializes the render target format parameter:
```c
void __fastcall FUN_004e1760(int *param_1)
{
    // ...
    param_1[0xf] = 1; // offset +0x3c (Engine Format ID = 1 -> D3DFMT_A8R8G8B8)
    // ...
}
```

When allocating intermediate post-processing buffers in `FUN_004e13d0`:
```c
iVar2 = FUN_00455970(param_1[3], param_1[4], param_1[0xf], 0x88);
```
The parameter `param_1[0xf]` is forwarded as the requested texture format. Because this was hardcoded to `1`, the engine allocated standard 8-bit integer buffers (`D3DFMT_A8R8G8B8`), restricting pixel values to the $[0.0, 1.0]$ interval between passes.

### Binary Modification
At virtual address `0x004e1793` in `SpellForce2.exe`:

```assembly
; Default assembly (Format ID 0x01 -> D3DFMT_A8R8G8B8):
004e1793  c7 46 3c 01 00 00 00    MOV dword ptr [ESI + 0x3c], 0x1

; Patched assembly (Format ID 0x1F -> D3DFMT_A16B16G16R16F):
004e1793  c7 46 3c 1f 00 00 00    MOV dword ptr [ESI + 0x3c], 0x1f
```

### Result
When initializing post-processing surfaces, the engine passes `EngineFormatID = 0x1F`. `FUN_0047b130(0x1F)` queries entry `0x00c5e54c` and returns `0x71` (`113` decimal), instructing the DirectX 9 runtime to allocate 64-bit floating point **`D3DFMT_A16B16G16R16F`** textures across all intermediate post-processing stages.

---

## 11. Format Selection Analysis: `A16B16G16R16F` vs `R32F` vs `A32B32G32R32F`

1. **`D3DFMT_R32F` (ID `0x1D` / `114`)**:
   - Single-channel 32-bit float ($R$ only).
   - Green, Blue, and Alpha channels are absent ($G=0, B=0, A=1$).
   - Using this format produces a monochromatic red display.
2. **`D3DFMT_A32B32G32R32F` (ID `0x20` / `116`)**:
   - Four-channel 32-bit float (128 bits per pixel / 16 bytes per pixel).
   - In Direct3D 9, hardware-accelerated **bilinear texture filtering (`Linear`) is not universally supported** on 32-bit float surfaces (`D3DUSAGE_QUERY_FILTER`). Unsupported hardware/drivers drop filtering to `Point` sampling, breaking separable blurs, distortion, and downsampling passes into blocky artifacts.
   - Doubles VRAM bandwidth overhead relative to 16-bit half-float formats.
3. **`D3DFMT_A16B16G16R16F` (ID `0x1F` / `113`)**:
   - Four-channel 16-bit float (64 bits per pixel / 8 bytes per pixel).
   - Hardware bilinear filtering (`Linear`), trilinear filtering, and post-pixel blending are fully standardized and supported across all modern and legacy DirectX 9 drivers.
   - Delivers dynamic range spanning $[6.10 \times 10^{-5}, 65504.0]$ with 11 stops of half-precision mantissa, eliminating clamping while preserving a 16-bit float Alpha channel for inter-pass seed transport.
