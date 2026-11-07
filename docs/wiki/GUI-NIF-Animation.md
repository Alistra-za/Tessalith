# NIF animation

The edit preview's clock, the Controllers tab and animation clips: how to watch a NIF's clips and edit its shader and alpha animation. For mod authors animating glow, colour or alpha. Opening a NIF for editing is on [Editing a NIF](GUI-NIF-Edit.md).

## Animation

The edit preview plays the shader and alpha controllers of the NIF you are editing. A NIF shown without **Edit** does not animate.

## Playback

When the edited NIF has a controller that animates, two rows sit under the 3D view: **Play** (or **Pause**), **Loop** ("Start again at the end"), **Speed** (0.1× to 4×; 1× is game speed), then the **Time** slider in seconds. Moving the slider jumps to that time and pauses. Without Loop, playback stops at the end, and Play starts again from the start. The slider runs from 0 to the end of the longest controller at its frequency (twice as long for a Reverse cycle). Below 30 frames a second the animation slows down instead of skipping. Playback only changes material values, so the mesh is never reloaded. A controller whose times are not numbers gets no controls, only the amber line "This animation has invalid times: playback is off." If you switch to the Textures view, the clock keeps its time and carries on when you come back. **In game** pauses the clock and hides these rows; after switching back to **Source**, press **Play** again.

## The Controllers tab

Each shader and alpha property lists its controllers, then **Add controller**. A float, colour or alpha-test controller has:
- A header with its name, its class, "Managed by a clip" when a clip drives it, and **Remove**. Remove unlinks the controller; the BSX Animated bit stays set. It is off for a controller a clip drives ("Driven by a clip: it cannot be removed.").
- **Variable**, **Cycle** (Loop, Reverse, Clamp), **Active**, **Play backwards**, **Frequency**, **Phase**, **Start**, **Stop** and the interpolator's value. You can change the Variable of a controller a clip drives: "Its clips follow the new variable."
- **Interpolation**: Linear or Quadratic. Linear to Quadratic adds tangents that keep the curve's shape; Quadratic to Linear drops them.
- The curve graph: the keys as handles, Quadratic tangent handles, and a cursor at the current time. Drag a key to change its time and value; drag a tangent handle up or down. One drag is one undo step. Click or drag the background to move the preview time. Times are never clamped: a key dragged past its neighbour gives the red error "Key times must not decrease". A curve too large to draw says "These keys are out of range for the graph."; the table still works.
- The key table: Time, Value (R, G and B for a colour), Forward and Backward for Quadratic keys, typed as exact numbers, and **Remove** for each key.
- A colour key also has a **Colour** swatch that opens a colour picker. Each drag in the picker is one undo step; numbers typed in the picker are one step until it closes. Channels you do not change keep their exact values. A key with a value outside 0-1 (a bright emissive colour) gets a greyed swatch: "Values outside 0-1: edit R, G and B directly."
- **Add key**: a new key after the last one, at the last key's time plus the average spacing between the keys (0.1 s with one key), with the last key's value. An empty curve starts at the start time. When no later time is possible (the last key is at the largest number), it is off: "No later time is possible for a new key."
- A controller's **Stop** follows its last key when you edit its keys (drag, table, Add key, Remove, Interpolation), in the same undo step; **Start** moves only when the first key goes before it.

TBC, XYZ and CONST keys are drawn but cannot be edited: "These keys are previewed, not edited." Keys you edit must be Linear or Quadratic, in time order and finite numbers. Otherwise a red error ("Key times must not decrease", "Only linear and quadratic keys can be authored", "Key values must be finite numbers") blocks the NIF's export. A key edit on a curve that another controller or clip shares makes a copy first.

**Colour strip.** A colour controller's curve graph has a strip under it that shows the colour the curve gives at each time; click or drag it to move the preview time. Where a channel is outside 0-1 (a bright emissive colour) the strip has a dashed band along its top, and that key's swatch has an outline. A float controller has no strip. The strip shows the stored colour, so it can look dimmer than the game, which multiplies it by Emissive Multiple.

## Add controller

On a shader property, pick **Float** or **Colour** and the **Variable**. An alpha property adds an alpha-test controller. The new controller has two Linear keys, at 0 and 1 s, that hold the property's current value, so nothing changes until you move a key; it loops at frequency 1. Adding sets the BSX Animated bit (a root without BSXFlags gets one). On the Extra data tab that box stays ticked and locked while a controller outside any clip animates the model. The button is off, with the reason on hover, when the root is not a node or the property cannot be found again in the file. On a shared alpha, Add controller splits it first (one undo step).

**PBR names.** On a PBR shape, as on the Shader tab, glossiness reads **Specular level**, specular strength **Roughness scale**, and specular colour **Coat colour**, **Subsurface colour** or "Specular colour (no effect on PBR)".

## Warnings

These come from an in-game check of each controller and show under the controller.
- Red: "Env map scale crashes the game on PBR materials." On a PBR shape, Env map scale (8) is greyed in both Variable lists with this text. A controller that already has it blocks the NIF's export, even when the source file had it.
- Amber, from the last In game simulation: on a shape that is not PBR but that PGPatcher makes PBR, an Env map scale controller gets "PGPatcher makes this shape PBR: its Env map scale controller will crash the game." It does not block export, since PGPatcher's output depends on the JSON at play time.
- Amber, no effect in game: "This controller has no effect: Specular is off when the mesh loads." and "…specular strength is 0 when the mesh loads." (glossiness, specular strength or colour on a non-PBR shape); "This controller has no effect: alpha test is off."; "Refraction strength has no visible effect in game."; "Two controllers drive <name>: only the last one plays."

## Clips

When the file has animation clips, a section **Clips (N · showing X)** sits at the top of the tab, open by default (X is the clip the Preview shows, or free chains). Its **Preview** list picks **Free chains** or one clip. A file with no clips has no such section. A clip shows alone: the other controllers keep their file value while it plays. Under **Free chains**, a clip named SpecialIdle plays by itself, as in the game, and the Time slider covers it. Any other controller a clip drives holds the first key of the first clip the manager lists for it, as the game does while that clip is not called. When nothing moves (only held values), the preview shows them with no Play or Time controls. Under a controller a clip drives, one weak line names the clip ("In clip SpecialIdle · Loop · 0 to 2 s"), and each clip shows its name, cycle, time span and text keys (one line; hover for all of them). The selected clip's keys get the same graph, table and Add key; the others say "Pick this clip in Preview to see and edit its keys." A clip with several interpolators for one controller cannot be edited ("This clip has several interpolators for this controller: they cannot be edited."). "No clip in this file drives it." marks a controller flagged as managed that no clip uses; it holds its property value.

## In game and Source while editing

While an edit is open, the toggle above the view still reads **In game** and **Source** (Source hovers "The edited source NIF, before PGPatcher"), and the edit opens on Source. In game runs Tessalith's PGPatcher simulation on your edited NIF and shows still materials (the clock pauses). PGPatcher's real output, made from the unedited NIF, is not shown while you edit. The line under the toggle, and the toggle's hover, say "Simulated as PGPatcher 2.3.1: loose JSON only, no modrules priority, read at the last scan". The JSON is read in the background when the edit opens ("Reading PGPatcher JSON…") and kept until the next scan or another scan source. A folder scan has no MO2 instance: "In game needs an MO2 instance: no PGPatcher JSON was read." For the selected shape a line says what happened: no JSON matches it, PGPatcher skips it (PG_IGNORE, amber), a file is not applied and why (amber; the next file can still match), it is already PBR, or which keys are not previewed. An amber line says when the shape carries a PG_STOCK block: PGPatcher first reverts such a shape to its recorded stock state, and the preview shows it as loaded. A red line says when PGPatcher 2.3 crashes on a JSON (a match key such as `match_diffuse` or `nif_filter` that is not a string, in any JSON the instance loads): the whole run stops, so every shape stays as edited and shows one red line per such file, including shapes PGPatcher would leave alone (no texture set, PG_IGNORE). The Textures view's 3D set preview shows the same red lines and no patched material. A red line also says when PGPatcher makes the shape PBR while it has an Env map scale controller, which crashes the game.

The simulation does not cover: JSON inside BSAs; PGPatcher's own mod priority (`modrules.json`; the MO2 order stands in); plugin alternate textures (TXST); the plugin and mesh-blocklist rules that decide which meshes PGPatcher patches; complex-material and parallax competition; shapes with no texture set; JSON changed on disk since it was read (rescan). It follows PGPatcher 2.3.1 (it does not simulate the PG_STOCK revert).
