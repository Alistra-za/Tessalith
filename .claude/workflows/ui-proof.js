export const meta = {
  name: 'ui-proof',
  description: 'Headless UI proof of one feature: owner-approved goals, blind explorer agents, frame checks and a verified, ranked report',
  whenToUse: 'After a GUI feature is built, to prove a new user can reach its goals and the screens hold up. Mode "goals": {mode: "goals", feature: "kebab-case", worktree: "<abs>", out: "<abs>\\\\.superpowers\\\\ui-proof\\\\<feature>", spec: "<path or [paths]>", owner: "<quoted owner messages>"}. The owner approves goals.md. Mode "run": {mode: "run", feature, worktree, out, goals: "<abs path to the approved goals.md>", sizes: ["1055x770","1400x790","1920x1080"], explore_size: "1400x790", max_goals: 5}',
  phases: [
    { title: 'Goals', detail: 'mode goals: one agent writes goals.md for the owner' },
    { title: 'Fixtures', detail: 'mode run: fixtures, oracles and one journey per goal (test code), then the shortest paths' },
    { title: 'Explore', detail: 'one blind agent per goal drives the app through the wrapper' },
    { title: 'Visual', detail: 'frame checks merged, then one agent reviews the frames against the rules' },
    { title: 'Verify', detail: 'one skeptic replays every finding and writes REPORT.md' },
  ],
}

// ui-proof: a headless UI proof of one feature. It drives the real app through crates/gui/tests/ui_drive (wrapper
// tools\ui-drive.ps1), which opens no window and writes a screenshot (NN.png), the AccessKit tree (NN.tree.txt) and the
// mechanical frame checks (NN.checks.json) after every step. Journeys are tests in crates/gui/tests/ui_proof.rs.
//
// Two modes, with the owner between them:
//   mode "goals"  one agent reads the spec and the owner's messages and writes <out>\goals.md (3-8 goals, each with a
//                 source, a fixture, one check line and a shortest path). 1 agent. Edits no code.
//                 The owner reads and approves goals.md. Nothing in mode "run" starts without that approval.
//   mode "run"    fixtures (1 agent: test code, one commit), explorers (one blind agent per goal, at most 5), visual (1),
//                 verify (1, writes <out>\REPORT.md and <out>\shots\). At most 8 agents.
//
// Args (all absolute paths): mode, feature (kebab-case), worktree, out (<repo>\.superpowers\ui-proof\<feature>).
//   goals mode: spec (a path or an array of paths), owner (quoted owner messages, optional).
//   run mode:   goals (path to the approved goals.md), sizes (default 1055x770, 1400x790, 1920x1080),
//               explore_size (default 1400x790), max_goals (default 5, never more than 5).
// Outputs: <out>\goals.md; <out>\paths\<gid>\<size>\ (shortest-path runs), <out>\explore\<gid>\ (explorer runs),
//   <out>\verify\<id>\ (replays), <out>\mechanical.json and .md, <out>\REPORT.md, <out>\shots\.
// Cost: the five explorers each run the app on WARP (the CPU renderer) at once, so the machine is busy while they work.

const a = args || {}
if (a.mode !== 'goals' && a.mode !== 'run') throw new Error('ui-proof needs args.mode "goals" or "run"')
for (const k of ['feature', 'worktree', 'out']) if (!a[k]) throw new Error(`ui-proof needs args.${k}`)
if (!/^[a-z0-9]+(-[a-z0-9]+)*$/.test(a.feature)) throw new Error('args.feature must be kebab-case')
if (a.mode === 'goals' && !a.spec) throw new Error('ui-proof goals mode needs args.spec')
if (a.mode === 'run' && !a.goals) throw new Error('ui-proof run mode needs args.goals (the approved goals.md)')

const W = a.worktree
const OUT = a.out
const WRAP = `${W}\\tools\\ui-drive.ps1`
const SHELL = `Use the PowerShell tool, not Bash. Never run \`python -\` heredocs. Never touch %APPDATA%\\sdc, %LOCALAPPDATA%\\sdc or the owner's game and mod folders. Never push. Run the app only through \`pwsh -NoProfile -File ${WRAP}\`. If you run cargo: only in ${W}, --offline, with CARGO_TARGET_DIR, CARGO_BUILD_JOBS=4 and CARGO_INCREMENTAL=0 as the plan's Global Constraints say, output captured to a file, never piped through head.`

const DRIVER = `The driver reads one JSON object per line (steps). A target names exactly one of "label" (exact), "starts" (prefix) or "placeholder", plus optional "role" and "index" (0 = first in reading order; needed when several nodes match). Use the labels in NN.tree.txt.
Ops, one example each:
{"op":"click","at":{"label":"Open"},"mods":"Ctrl+Shift"}   (also double_click, right_click, hover; mods is optional)
{"op":"type","text":"abc","at":{"placeholder":"Search"}}   (at clicks the field first)
{"op":"key","key":"Ctrl+K"}   (Enter, Escape, Ctrl+Shift+Z ...)
{"op":"drag","from":{"label":"A"},"to":{"label":"B"}}
{"op":"scroll","at":{"label":"List"},"dir":"down","times":3}   (dir: up, down, left, right, into_view)
{"op":"resize","w":1100,"h":700}
{"op":"settle","frames":4}
{"op":"pick","path":"textures"}   (answers the next file dialog with a folder inside the project; none queued = Cancel)
{"op":"expect","fact":"/selection","equals":"set:architecture/stone/stonewall"}   (also "contains": v, or "absent": true; fact is a JSON Pointer into NN.state.json)
{"op":"check","name":"sets_pills_match_their_counts"}
Each run writes NN.png (the screen after step NN; 00 is the start) and NN.tree.txt (role, label, position, with focused/disabled/hovered/clipped/offscreen marks) into the output folder, plus NN.state.json (the app's model, not the screen), NN.checks.json and run.json.`

const GOALS = {
  type: 'object',
  properties: {
    path: { type: 'string' },
    goals: { type: 'array', items: { type: 'object', properties: {
      id: { type: 'string' }, sentence: { type: 'string' }, source: { type: 'string' },
    }, required: ['id', 'sentence', 'source'] } },
  },
  required: ['path', 'goals'],
}

const FIXTURES = {
  type: 'object',
  properties: {
    commit: { type: 'string' },
    goals: { type: 'array', items: { type: 'object', properties: {
      id: { type: 'string' }, sentence: { type: 'string' }, fixture: { type: 'string' }, check: { type: 'string' },
      shortest_steps: { type: 'number' }, path_ok: { type: 'boolean' }, path_error: { type: 'string' },
      blocked: { type: 'string' },
      runs: { type: 'array', items: { type: 'object', properties: {
        size: { type: 'string' }, exit: { type: 'number' }, reached_at: { type: ['number', 'null'] }, dir: { type: 'string' },
      }, required: ['size', 'exit', 'dir'] } },
    }, required: ['id', 'sentence', 'fixture', 'check', 'shortest_steps', 'path_ok', 'runs'] } },
  },
  required: ['commit', 'goals'],
}

const EXPLORER = {
  type: 'object',
  properties: {
    goal: { type: 'string' }, reached: { type: 'boolean' }, steps_used: { type: 'number' },
    log: { type: 'array', items: { type: 'object', properties: {
      n: { type: 'number' }, step: { type: 'string' }, saw: { type: 'string' }, why: { type: 'string' },
    }, required: ['n', 'step', 'saw', 'why'] } },
    dead_ends: { type: 'array', items: { type: 'object', properties: { n: { type: 'number' }, what: { type: 'string' } }, required: ['n', 'what'] } },
    wrong_clicks: { type: 'array', items: { type: 'object', properties: { n: { type: 'number' }, label: { type: 'string' }, why: { type: 'string' } }, required: ['n', 'label', 'why'] } },
    hovers_needed: { type: 'array', items: { type: 'object', properties: { n: { type: 'number' }, label: { type: 'string' }, text: { type: 'string' } }, required: ['n', 'label', 'text'] } },
    dir: { type: 'string' },
  },
  required: ['goal', 'reached', 'steps_used', 'log', 'dead_ends', 'wrong_clicks', 'hovers_needed', 'dir'],
}

const VISUAL = {
  type: 'object',
  properties: {
    findings: { type: 'array', items: { type: 'object', properties: {
      id: { type: 'string' }, where: { type: 'string' }, what: { type: 'string' }, why: { type: 'string' }, fix: { type: 'string' },
      rule: { type: 'string' }, frame: { type: 'string' },
      repro: { type: 'object', properties: { dir: { type: 'string' }, step: { type: 'number' } }, required: ['dir', 'step'] },
    }, required: ['id', 'where', 'what', 'why', 'fix', 'rule', 'frame', 'repro'] } },
  },
  required: ['findings'],
}

const VERIFY = {
  type: 'object',
  properties: {
    report: { type: 'string' }, kept: { type: 'number' }, dropped: { type: 'number' }, intent_failures: { type: 'number' },
  },
  required: ['report', 'kept', 'dropped', 'intent_failures'],
}

if (a.mode === 'goals') {
  phase('Goals')
  const specs = [].concat(a.spec).join(', ')
  const res = await agent(
    `Write the goals for the owner's approval for the feature "${a.feature}". You edit no code.
Read: the spec (${specs}); the owner's messages: ${a.owner ? JSON.stringify(a.owner) : '(none given)'}; the driver header in ${W}\\crates\\gui\\tests\\ui_drive\\main.rs.
Run \`pwsh -NoProfile -File ${WRAP} -List\` for the fixtures and checks (run -Build first if ${W}\\.claude\\scratch\\ui_drive.exe is missing; that needs CARGO_TARGET_DIR). You may run the driver on the fixture \`demo\` to learn exact labels, with -Out under ${OUT}\\labels.
${DRIVER}
Write ${OUT}\\goals.md (create the folder) in exactly this shape:
# ${a.feature}: goals for the owner's approval
Spec: <paths>. Owner: "<quote>" (<date>)
## g1 — <the goal in the owner's words>
- Source: <path:line> or "<owner quote>"
- Fixture: demo | new: <what it must hold>
- Check: \`<one expect or check line>\` (a new check: its predicate in words, with its source)
- Shortest path: a \`\`\`jsonl block of steps
(more goals, g2, g3 ...)
## Filters and controls
| Control | Option | Shows exactly | Source |
Rules: 3 to 8 goals. A goal without a source is not written. A goal from taste is not written. Each goal is something a user does, stated as an outcome the app's state can show.
${SHELL}
Return the file path and the goals (id, sentence, source).`,
    { label: 'goals', phase: 'Goals', schema: GOALS })
  if (!res || !res.goals || !res.goals.length) throw new Error('goals agent returned no goals')
  log(`goals mode: 1 agent, ${res.goals.length} goals in ${res.path}`)
  return { mode: 'goals', goals: res.path, count: res.goals.length, next: 'The owner approves goals.md; then run mode "run"' }
}

// ---- mode run ----
const SIZES = a.sizes && a.sizes.length ? a.sizes : ['1055x770', '1400x790', '1920x1080']
const EXPLORE_SIZE = a.explore_size || '1400x790'
const MAX = Math.min(a.max_goals || 5, 5)

phase('Fixtures')
const fx = await agent(
  `Fixtures and journeys for "${a.feature}". The goals in ${a.goals} are fixed and approved by the owner: never rewrite one. A goal you cannot check as written is returned with \`blocked\` set to the reason (and path_ok false).
You write test code only, in ${W}: fixtures in crates\\gui\\tests\\ui_drive\\fixtures.rs, oracles in oracles.rs (with \`filter_matches\`, one per filter option in the goals' table), one journey per goal in crates\\gui\\tests\\ui_proof\\${a.feature.replace(/-/g, '_')}.rs (its shortest path ending in its check) plus the \`#[path]\` line in crates\\gui\\tests\\ui_proof.rs. Never touch crates\\*\\src.
Then: pwsh -NoProfile -File ${WRAP} -Build; run each goal's shortest path at each size (${SIZES.join(', ')}) with -Goal '<its check line as one JSON step>' into ${OUT}\\paths\\<goal id>\\<size> (the steps file is the goal's shortest path; its last line is the check). A goal whose path fails (exit 1, or no GOAL REACHED) has path_ok false and path_error set: that is an intent failure for the report, not yours to fix by editing the goal.
Also write each goal's check line (one JSON step, nothing else) to ${OUT}\\explore\\<goal id>\\goal.json, so an explorer's command line never shows it.
Gate, outputs to files: cargo fmt --all --check; cargo clippy --offline --workspace --all-targets -- -D warnings; cargo clippy --offline --workspace -- -D warnings; cargo test --offline -p sdc-gui --test ui_proof -- --test-threads=4. Fix test code until the gate passes. Make one commit of explicit paths with the message \`test(ui-proof): ${a.feature} fixtures and journeys\`.
${DRIVER}
${SHELL}
Return the commit, and per goal: id, sentence, fixture, check, shortest_steps (the step count before the check), path_ok, path_error, blocked, runs [{size, exit, reached_at, dir}].`,
  { label: 'fixtures', phase: 'Fixtures', schema: FIXTURES })
if (!fx || !fx.goals || !fx.goals.length) throw new Error('fixtures agent returned no goals')
const usable = fx.goals.filter(g => !g.blocked)
const dropped = fx.goals.slice(MAX)
for (const g of fx.goals.filter(g => g.blocked)) log(`goal ${g.id} blocked: ${g.blocked}`)
if (fx.goals.length > MAX) log(`explore: ${dropped.length} goal(s) not explored (max_goals ${MAX}): ${dropped.map(g => g.id).join(', ')}`)
const picked = usable.slice(0, MAX)
const failures = fx.goals.filter(g => !g.path_ok)

phase('Explore')
const explored = (await pipeline(picked, (g) => {
  const dir = `${OUT}\\explore\\${g.id}`
  const cap = Math.min(3 * g.shortest_steps, 25)
  return agent(
    `You are a new user of a desktop app. You have not seen its code, documentation or wiki and must not look: do not open, list or search any file except the ones named here. Your goal: "${g.sentence}". You use the app only through \`pwsh -NoProfile -File ${WRAP} -Fixture ${g.fixture} -Size ${EXPLORE_SIZE} -GoalFile ${dir}\\goal.json -Steps ${dir}\\steps.jsonl -Out ${dir}\`. Write your steps to \`${dir}\\steps.jsonl\` and run the command; each run replays every step from a fresh start and writes NN.png (the screen after step NN; 00 is the start) and NN.tree.txt (what is on that screen: role, label, position). Read only those two kinds of file and the command's output. Add one step at a time. When the output says GOAL REACHED, stop; stop also after ${cap} steps. For each step, record what you saw and why you chose it. A dead end is a step you later undid or abandoned; a wrong click is one that did not move you toward the goal; a hover you needed is one whose text told you what to do.

${DRIVER}
Use the PowerShell tool. Return the schema: goal, reached, steps_used, log [{n, step, saw, why}], dead_ends [{n, what}], wrong_clicks [{n, label, why}], hovers_needed [{n, label, text}], dir ("${dir}").`,
    { label: `explore ${g.id}`, phase: 'Explore', schema: EXPLORER })
})).filter(Boolean)

phase('Visual')
const visual = await agent(
  `Visual review of "${a.feature}", read-only except the files named here. First run \`pwsh -NoProfile -File ${WRAP} -Collect ${OUT}\`, then read ${OUT}\\mechanical.md so you do not repeat it. Load the \`egui-ui-rules\` and \`egui-emil-design-eng\` skills.
Look at the shortest-path frames at every size (${OUT}\\paths\\<goal id>\\<size>\\NN.png) and, in each explorer run, the frames at its dead ends and wrong clicks (${OUT}\\explore\\<goal id>\\). Explorer results: ${JSON.stringify(explored.map(e => ({ goal: e.goal, dir: e.dir, reached: e.reached, dead_ends: e.dead_ends, wrong_clicks: e.wrong_clicks })))}
Every finding names a rule (an egui-ui-rules item, a DECISIONS ID from ${W}\\.claude\\DECISIONS.md, or a goal id) or a measured cost (steps, pixels); taste without either is not a finding. frame is the PNG path; repro is the run folder and the step number.
${SHELL}`,
  { label: 'visual', phase: 'Visual', schema: VISUAL })
if (!visual) throw new Error('visual reviewer returned nothing')

phase('Verify')
const verify = await agent(
  `You are the skeptic for the UI proof of "${a.feature}" (worktree ${W}, fixtures commit ${fx.commit}, sizes ${SIZES.join(', ')}, explorer size ${EXPLORE_SIZE}). Write ${OUT}\\REPORT.md.
Inputs. Path failures (intent failures): ${JSON.stringify(failures.map(g => ({ id: g.id, sentence: g.sentence, error: g.path_error })))}. Blocked goals: ${JSON.stringify(fx.goals.filter(g => g.blocked).map(g => ({ id: g.id, why: g.blocked })))}. Goals not explored: ${JSON.stringify(dropped.map(g => g.id))}. Shortest paths: ${JSON.stringify(picked.map(g => ({ id: g.id, shortest_steps: g.shortest_steps })))}. Explorer results: ${JSON.stringify(explored)}. Mechanical findings: ${OUT}\\mechanical.md. Visual findings: ${JSON.stringify(visual.findings)}.
Method: re-run each visual finding with \`pwsh -NoProfile -File ${WRAP} -Replay <its repro.dir> -Upto <its repro.step> -Out ${OUT}\\verify\\<id>\` and keep it only if the new frame, tree or checks still show it and it names a rule or a measured cost. Drop it when unsure. Drop mechanical findings the screenshot shows are harmless, with the reason. Grep ${W}\\.claude\\DECISIONS.md for the area (never read it whole): a finding that contradicts a ruling names it. Copy each kept finding's frame to ${OUT}\\shots\\<NN>-<id>.png.
REPORT.md, in this shape: a header (feature, worktree commit, fixtures, sizes, adapter and whether the screenshots are comparable, from run.json); **Goals** (Goal | Reached | Steps: explorer / shortest | Dead ends | Hovers needed); **Findings, ranked** (# | Where | What happens | Why it hurts | Proposed fix | Ruling), intent failures first, then by measured cost and by rules that block use (off-window, overlap), then polish, each with its screenshot; **Painted-only text** (the widgets whose text is painted rather than in the tree, for the follow-up plan); **Dropped** (one line each, with the reason).
${SHELL}
Return report (the path), kept, dropped, intent_failures.`,
  { label: 'verify', phase: 'Verify', schema: VERIFY })
if (!verify) throw new Error('verifier returned nothing')

log(`run mode: ${2 + explored.length + 1} agents (fixtures 1, explorers ${explored.length}, visual 1, verify 1)`)
return { mode: 'run', report: verify.report, kept: verify.kept, dropped: verify.dropped, goals: fx.goals.map(g => g.id) }
