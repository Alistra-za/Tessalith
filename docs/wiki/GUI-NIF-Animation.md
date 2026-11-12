# NIF animation

For mod authors animating glow, colour or alpha: play a NIF's clips and edit the keys of its shader and alpha controllers. The edit preview plays the controllers of the NIF you are editing; a NIF shown without **Edit** does not animate. Opening a NIF for editing, and the **In game** simulation, are on [Editing a NIF](GUI-NIF-Edit.md). Clips can't be created, removed or renamed; the keys of the clip shown in **Preview** can be edited.

## Playback

When the edited NIF has a controller that animates, two rows sit under the 3D view: **Play** (or **Pause**), **Loop** ("Start again at the end"), **Speed** (0.1× to 4×; 1× is game speed), then the **Time** slider in seconds. Moving the slider jumps to that time and pauses. Without Loop, playback stops at the end, and Play starts again from the start. The slider runs from 0 to the end of the longest controller at its frequency (twice as long for a Reverse cycle). A controller whose times are not numbers gets no controls, only the amber line "This animation has invalid times: playback is off." **In game** pauses the clock and hides these rows; press **Play** again after switching back to **Source**.

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
- Amber, from the last In game simulation: the same PGPatcher warning as on the shape cards ([In game and Source while editing](GUI-NIF-Edit.md#in-game-and-source-while-editing)), shown on the Env map scale controller. It does not block export, since PGPatcher's output depends on the JSON at play time.
- Amber, no effect in game: "This controller has no effect: Specular is off when the mesh loads." and "…specular strength is 0 when the mesh loads." (glossiness, specular strength or colour on a non-PBR shape); "This controller has no effect: alpha test is off."; "Refraction strength has no visible effect in game."; "Two controllers drive <name>: only the last one plays."

## Clips

When the file has animation clips, a **Clips** section sits at the top of the tab. Its **Preview** list picks **Free chains** or one clip. A clip shows alone: the other controllers keep their file value while it plays. Under **Free chains**, a clip named SpecialIdle plays by itself, as in the game, and the Time slider covers it. Any other controller a clip drives holds the first key of the first clip the manager lists for it, as the game does while that clip is not called. When nothing moves (only held values), the preview shows them with no Play or Time controls. Under a controller a clip drives, one weak line names the clip ("In clip SpecialIdle · Loop · 0 to 2 s"), and each clip shows its name, cycle, time span and text keys (one line; hover for all of them). The selected clip's keys get the same graph, table and Add key; the others say "Pick this clip in Preview to see and edit its keys." A clip with several interpolators for one controller cannot be edited ("This clip has several interpolators for this controller: they cannot be edited."). A key edit made on a clip is refused as a conflict when another clip has since taken its place (after the source file changes); the Preview then returns to Free chains rather than show the other clip. A file whose own manager has no clips shows "This file has a NiControllerManager with no clips: the game crashes on it." as a warning. "No clip in this file drives it." marks a controller flagged as managed that no clip uses; it holds its property value.
