import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { ALIAS_RE, ALL_CATALOG_CATEGORIES, INTERACT_LIMITS } from "../../command_inputs.ts";
import { controlCamera, inspectEntities, selectEntities } from "../../capabilities/observation.ts";
import { captureFixed, presentScreenshotResult, startRecording } from "../../capabilities/media.ts";
import type { InteractTailnetPreview } from "../../tailnet_preview.ts";
import { InteractError } from "../../service_contract.ts";
import type {
  AliasEntry, EntityRef, InteractSession, JsonObject, ResolvedEntityRef, ServiceInput, SpawnInput,
} from "../../service_contract.ts";

export async function executeLabCommand(
  command: string,
  session: InteractSession,
  input: ServiceInput,
  artifactPreview: InteractTailnetPreview | null,
  workspaceRoot: string,
) {
  const sessionId = session.sessionId;
  if (command === "reset") {
    const before = await aliasSnapshots(session);
    const result = await session.driver.reset();
    return { sessionId, result, ...await reconcileAliasesAfterReset(session, before) };
  }
  if (command === "catalog") return { sessionId, ...projectCatalog(await session.driver.catalog(), input.categories) };
  if (command === "spawn") return spawn(session, input.spawns, input.details === true);
  if (command === "update") return update(session, input.updates || (input.update ? [input.update] : []));
  if (command === "remove") {
    const resolved = await resolveEntityReferences(session, input.refs || []);
    const result = await session.driver.remove(resolved.map((entry) => entry.id));
    const removed = resolved.map((entry) => ({ id: entry.id, alias: aliasForEntity(session.aliases, entry.id) }));
    for (const entry of resolved) clearAliasesForEntity(session.aliases, entry.id);
    return { sessionId, removed, result };
  }
  if (command === "order") return order(session, input);
  if (command === "time") return { sessionId, result: await session.driver.time(input.control || {}) };
  if (command === "inspect") return inspect(session, input);
  if (command === "select") return select(session, input);
  if (command === "camera") return camera(session, input.camera || {});
  if (command === "screenshot") return screenshot(session, input, artifactPreview);
  if (command === "export") return exportArtifact(workspaceRoot, session, input);
  if (command === "import") return importArtifact(workspaceRoot, session, input);
  if (command === "artifact-inspect") return inspectArtifact(workspaceRoot, session, input);
  if (command === "record-start") return startRecording(session, input, artifactPreview);
  if (command === "capture-fixed") return captureFixed(session, input, artifactPreview);
  throw new InteractError("unknownCommand", `Unknown Lab command ${command}.`);
}

async function spawn(session: InteractSession, spawns: SpawnInput[] = [], includeDetails = false) {
  validateSpawnAliases(session, spawns);
  const catalog = await session.driver.catalog();
  const playerIds = new Set(objectArray(catalog.players).map((player) => Number(player.id)));
  const spawnableKinds = new Set(flattenFactions(catalog.factions, "units").concat(flattenFactions(catalog.factions, "buildings")));
  for (const spec of spawns) {
    if (!playerIds.has(spec.owner)) throw new InteractError("unknownPlayer", `Player ${spec.owner} is not available.`);
    if (!spawnableKinds.has(spec.kind)) throw new InteractError("invalidKind", `${spec.kind} is not spawnable.`);
  }
  const response = await session.driver.spawn(spawns.map(({ alias: _alias, ...spec }) => spec));
  const entities = objectArray(response.entities);
  const result = asJsonObject(response.result);
  const outcome = asJsonObject(result.outcome);
  const outcomes = objectArray(outcome.items);
  const staged: Array<{ spec: SpawnInput; id: number; entity: JsonObject | null }> = [];
  for (let index = 0; index < spawns.length; index += 1) {
    const spec = spawns[index];
    const entity = entities[index] || null;
    const candidateId = entity?.id ?? asJsonObject(outcomes[index]?.outcome).entityId;
    if (!Number.isInteger(candidateId) || Number(candidateId) <= 0) throw new InteractError("missingEntityId", "Spawn did not return an entity id.");
    staged.push({ spec, id: Number(candidateId), entity });
  }
  for (const { spec, id } of staged) if (spec.alias) session.aliases.set(spec.alias, id);
  const summary = boundedResponseSummary(staged.map(({ spec, id }, index) => ({
    index,
    alias: spec.alias || null,
    id,
  })));
  const compact = {
    sessionId: session.sessionId,
    spawned: summary,
    snapshotTick: findSnapshotTick(response),
  };
  if (!includeDetails) return compact;
  const results = staged.map(({ spec, id, entity }) => ({
    alias: spec.alias || null,
    id,
    entity: decorateEntity(entity, session.aliases),
  }));
  return { ...compact, results, result };
}

async function update(session: InteractSession, values: JsonObject[] = []) {
  const catalog = await session.driver.catalog();
  const entityValues = values.filter((value) => ["move", "owner"].includes(String(value.operation)));
  const resolvedEntities = entityValues.length
    ? await resolveEntityReferences(session, entityValues.map((value) => value.entity as EntityRef))
    : [];
  let resolvedIndex = 0;
  const operations: JsonObject[] = [];
  for (const value of values) {
    assertKnownPlayer(catalog, value.playerId ?? value.owner);
    let operation = value;
    if (value.operation === "move") {
      const entity = resolvedEntities[resolvedIndex++];
      operation = { operation: "move", entityId: entity.id, x: value.x, y: value.y };
    } else if (value.operation === "owner") {
      const entity = resolvedEntities[resolvedIndex++];
      operation = { operation: "reassign", entityId: entity.id, owner: value.owner };
    } else if (value.operation === "research" && !flattenFactions(catalog.factions, "upgrades").includes(String(value.upgrade))) {
      throw new InteractError("invalidUpgrade", `${value.upgrade} is not an available lab upgrade.`);
    }
    operations.push(operation);
  }
  return { sessionId: session.sessionId, result: await session.driver.update(operations) };
}

async function order(session: InteractSession, { playerId, command, ignoreCommandLimits = false }: ServiceInput) {
  if (typeof playerId !== "number" || !command) throw new InteractError("invalidInput", "order requires playerId and command.");
  const catalog = await session.driver.catalog();
  assertKnownPlayer(catalog, playerId);
  validateCommandCatalog(command, catalog);
  const { command: resolvedCommand, resolved } = await resolveCommand(session, command);
  const result = await session.driver.order({ playerId, command: resolvedCommand, ignoreCommandLimits });
  return { sessionId: session.sessionId, command: resolvedCommand, resolved, result };
}

async function inspect(session: InteractSession, { refs, kinds, owners, cameraViewport, limit }: ServiceInput) {
  const resolved = refs ? await resolveEntityReferences(session, refs) : [];
  const response = await inspectEntities(session, {
    ids: resolved.map((entry) => entry.id), kinds, owners,
    cameraViewport: cameraViewport === true, limit: limit || 25,
  });
  return {
    sessionId: session.sessionId,
    entities: objectArray(response.entities).map((entity) => decorateEntity(entity, session.aliases)),
    players: response.players || [], room: response.room || null, camera: response.camera || null,
    cameraViewport: response.cameraViewport || null,
    cameraWorldBounds: response.cameraWorldBounds || null, selection: Array.isArray(response.selection) ? response.selection : [],
    truncated: response.truncated === true,
    totalMatching: Number.isInteger(response.totalMatching) ? response.totalMatching : 0,
  };
}

async function select(session: InteractSession, { refs = [] }: ServiceInput) {
  const resolved = await resolveEntityReferences(session, refs); const response = await selectEntities(session, resolved.map((entry) => entry.id));
  return { sessionId: session.sessionId, selection: Array.isArray(response.selection) ? response.selection : [],
    entities: objectArray(response.entities).map((entity) => decorateEntity(entity, session.aliases)),
  };
}

async function camera(session: InteractSession, value: JsonObject = {}) {
  let command: JsonObject = value;
  if (value.action === "focus") {
    const resolved = await resolveEntityReferences(session, value.refs as EntityRef[]);
    command = { action: "focus", entityIds: resolved.map((entry) => entry.id), padding: value.padding };
  }
  return controlCamera(session, command);
}

async function screenshot(session: InteractSession, { name = "scene", presentation = "clean", viewport, subjects }: ServiceInput, artifactPreview: InteractTailnetPreview | null) {
  const resolved = subjects ? await resolveEntityReferences(session, subjects) : [];
  const inspected = resolved.length
    ? await session.driver.inspect({ ids: resolved.map((entry) => entry.id), limit: resolved.length })
    : { entities: [] };
  const entitiesById = new Map(objectArray(inspected.entities).map((entity) => [Number(entity.id), entity]));
  const subjectSummaries = resolved.map((entry) => decorateEntity(entitiesById.get(entry.id), session.aliases));
  const capture = await session.driver.screenshot({
    sessionId: session.sessionId, name, presentation, viewport,
    subjectIds: resolved.map((entry) => entry.id), subjectSummaries,
    request: { command: "screenshot", sessionId: session.sessionId, name, presentation, viewport, subjects: resolved },
  });
  const result = {
    sessionId: session.sessionId,
    pngPath: capture.pngPath,
    manifestPath: capture.manifestPath,
    presentation: capture.presentation,
    image: {
      mimeType: capture.image.mimeType,
      bytes: capture.image.bytes,
      width: capture.image.width,
      height: capture.image.height,
    },
    readiness: capture.readiness,
  };
  return presentScreenshotResult(result, artifactPreview);
}

async function exportArtifact(workspaceRoot: string, session: InteractSession, { kind, name = "", reproduction = false }: ServiceInput) {
  if (kind !== "setup" && kind !== "replay") throw new InteractError("invalidArtifactKind", "export requires setup or replay kind.");
  const artifactId = `artifact_${crypto.randomUUID().replaceAll("-", "")}`;
  const directory = artifactDirectory(workspaceRoot);
  fs.mkdirSync(directory, { recursive: true });
  let artifact;
  if (kind === "setup") {
    artifact = (await session.driver.exportSetup(name)).scenario;
  } else {
    artifact = JSON.parse((await session.driver.exportReplay(name)).bytes.toString("utf8"));
  }
  const bytes = Buffer.from(`${JSON.stringify(artifact, null, 2)}\n`);
  if (bytes.length > INTERACT_LIMITS.maxArtifactBytes) {
    throw new InteractError("artifactTooLarge", "Artifact exceeds the 8 MiB local file bound.");
  }
  const artifactPath = path.join(directory, `${artifactId}.${kind}.json`);
  const sidecarPath = path.join(directory, `${artifactId}.aliases.json`);
  const aliases = [...session.aliases].map(([alias, id]) => ({ alias, id }));
  const sidecar = {
    schemaVersion: 1,
    artifactId,
    kind,
    artifactFile: path.basename(artifactPath),
    aliases,
    reproduction: reproduction ? reproductionSummary(kind, artifactId, aliases) : null,
  };
  fs.writeFileSync(artifactPath, new Uint8Array(bytes), { mode: 0o600 });
  fs.writeFileSync(sidecarPath, `${JSON.stringify(sidecar, null, 2)}\n`, { mode: 0o600 });
  return {
    sessionId: session.sessionId,
    artifactId,
    kind,
    path: artifactPath,
    sidecarPath,
    bytes: bytes.length,
    ...artifactSummary(kind, artifact, aliases),
    reproduction: sidecar.reproduction,
  };
}

async function importArtifact(workspaceRoot: string, session: InteractSession, selector: ServiceInput) {
  const selected = readArtifact(workspaceRoot, selector);
  let importResult;
  let reconciliation;
  if (selected.kind === "setup") {
    importResult = await session.driver.importSetup(selected.artifact);
    reconciliation = await reconcileImportedAliases(session, selected.aliases, importResult.entityIdMap || []);
  } else {
    importResult = await session.driver.importReplay(selected.bytes);
    reconciliation = await validateImportedAliases(session, selected.aliases);
  }
  session.sceneIdentity = { source: "artifact", artifactId: selected.artifactId, kind: selected.kind, path: selected.path };
  session.sceneRevision = 0;
  const compact = {
    sessionId: session.sessionId,
    artifactId: selected.artifactId,
    kind: selected.kind,
    path: selected.path,
    imported: true,
    tick: (await session.driver.status()).snapshotTick ?? null,
    aliases: {
      restored: boundedResponseSummary(reconciliation.restored),
      stale: boundedResponseSummary(reconciliation.stale),
    },
    validation: { ok: true, authority: selected.kind === "setup" ? "checkpoint import" : "replay room rebuild" },
  };
  return selector.details === true
    ? { ...compact, aliases: reconciliation, result: importResult }
    : compact;
}

async function inspectArtifact(workspaceRoot: string, session: InteractSession, selector: ServiceInput) {
  const selected = readArtifact(workspaceRoot, selector);
  const aliasValidation = await validateAliasesAgainstArtifact(session, selected.kind, selected.artifact, selected.aliases);
  return {
    sessionId: session.sessionId,
    artifactId: selected.artifactId,
    kind: selected.kind,
    path: selected.path,
    sidecarPath: selected.sidecarPath,
    bytes: selected.bytes.length,
    ...artifactSummary(selected.kind, selected.artifact, selected.aliases),
    aliases: aliasValidation,
    validation: {
      ok: true,
      schema: selected.kind === "setup" ? "LabCheckpointScenarioV1" : "LabReplayArtifactV1",
      scope: "boundedLocalShape",
      authoritative: false,
      authoritativeValidationOnImport: true,
    },
  };
}

function artifactDirectory(workspaceRoot: string) {
  return path.join(workspaceRoot, "target", "interact", "lab", "artifacts");
}

function readArtifact(workspaceRoot: string, selector: ServiceInput) {
  const kind = selector.kind || null;
  const artifactId = selector.artifactId || null;
  const requestedPath = selector.path || null;
  const directory = artifactDirectory(workspaceRoot);
  let artifactPath: string | undefined;
  if (artifactId) {
    if (!/^artifact_[a-f0-9]{32}$/.test(artifactId)) throw new InteractError("invalidArtifactId", "artifactId is invalid.");
    const candidates = kind
      ? [path.join(directory, `${artifactId}.${kind}.json`)]
      : ["setup", "replay"].map((candidateKind) => path.join(directory, `${artifactId}.${candidateKind}.json`));
    artifactPath = candidates.find((candidate) => fs.existsSync(candidate));
    if (!artifactPath) throw new InteractError("artifactNotFound", "Artifact id does not exist in this worktree.");
  } else {
    if (!requestedPath) throw new InteractError("invalidArtifactPath", "Artifact import requires artifactId or path.");
    if (/^[A-Za-z][A-Za-z0-9+.-]*:/.test(requestedPath)) throw new InteractError("unsafeArtifactPath", "Artifact URLs are not accepted.");
    artifactPath = path.resolve(workspaceRoot, requestedPath);
  }
  const rootPrefix = `${fs.realpathSync(path.join(workspaceRoot, "target", "interact", "lab"))}${path.sep}`;
  let realPath;
  try { realPath = fs.realpathSync(artifactPath); } catch { throw new InteractError("artifactNotFound", "Artifact path does not exist."); }
  if (!realPath.startsWith(rootPrefix)) throw new InteractError("unsafeArtifactPath", "Artifact path must stay beneath target/interact/lab/.");
  const match = path.basename(realPath).match(/^(artifact_[a-f0-9]{32})\.(setup|replay)\.json$/);
  if (!match) throw new InteractError("invalidArtifactPath", "Artifact filename is not an Interact artifact.");
  const resolvedKind = match[2];
  if (kind && kind !== resolvedKind) throw new InteractError("artifactKindMismatch", "Requested artifact kind does not match the file.");
  const bytes = readBoundedFile(realPath, INTERACT_LIMITS.maxArtifactBytes, "artifactTooLarge", "Artifact exceeds 8 MiB.");
  let artifact: unknown;
  try { artifact = JSON.parse(bytes.toString("utf8")) as unknown; } catch { throw new InteractError("invalidArtifact", "Artifact is not valid JSON."); }
  validateArtifactShape(resolvedKind, artifact);
  const sidecarPath = path.join(path.dirname(realPath), `${match[1]}.aliases.json`);
  const aliases = readAliasSidecar(sidecarPath, match[1], resolvedKind);
  return { artifactId: match[1], kind: resolvedKind, path: realPath, sidecarPath, bytes, artifact, aliases };
}

function validateArtifactShape(kind: string, artifact: unknown): asserts artifact is JsonObject {
  if (!isJsonObject(artifact)) throw new InteractError("invalidArtifact", "Artifact must be an object.");
  if (kind === "setup") {
    if (artifact.schemaVersion !== 1 || artifact.kind !== "labCheckpointScenario" || typeof artifact.checkpointPayload !== "string") {
      throw new InteractError("incompatibleArtifact", "Only LabCheckpointScenarioV1 setup artifacts are accepted.");
    }
  } else if (artifact.schema !== "rts.labReplay" || artifact.schemaVersion !== 1 || artifact.kind !== "labReplay" || !Array.isArray(artifact.operations)) {
    throw new InteractError("incompatibleArtifact", "Only LabReplayArtifactV1 replay artifacts are accepted.");
  }
}

function readAliasSidecar(sidecarPath: string, artifactId: string, kind: string): AliasEntry[] {
  if (!fs.existsSync(sidecarPath)) return [];
  let sidecar: unknown;
  try {
    const bytes = readBoundedFile(
      sidecarPath,
      INTERACT_LIMITS.maxAliasSidecarBytes,
      "invalidAliasSidecar",
      "Alias sidecar exceeds 64 KiB.",
    );
    sidecar = JSON.parse(bytes.toString("utf8")) as unknown;
  } catch (error) {
    if (error instanceof InteractError) throw error;
    throw new InteractError("invalidAliasSidecar", "Alias sidecar is invalid JSON.");
  }
  if (!isJsonObject(sidecar) || sidecar.schemaVersion !== 1 || sidecar.artifactId !== artifactId || sidecar.kind !== kind || !Array.isArray(sidecar.aliases) || sidecar.aliases.length > INTERACT_LIMITS.maxAliases) {
    throw new InteractError("invalidAliasSidecar", "Alias sidecar identity or bounds are invalid.");
  }
  const seen = new Set<string>();
  return sidecar.aliases.map((entry: unknown) => {
    if (!isJsonObject(entry) || typeof entry.alias !== "string" || !ALIAS_RE.test(entry.alias) || !Number.isInteger(entry.id) || Number(entry.id) <= 0 || seen.has(entry.alias)) throw new InteractError("invalidAliasSidecar", "Alias sidecar contains an invalid or duplicate entry.");
    seen.add(entry.alias);
    return { alias: entry.alias, id: Number(entry.id) };
  });
}

function readBoundedFile(filePath: string, maxBytes: number, code: string, message: string) {
  let size;
  try { size = fs.statSync(filePath).size; } catch { throw new InteractError("artifactNotFound", "Artifact path does not exist."); }
  if (size > maxBytes) throw new InteractError(code, message);
  const bytes = fs.readFileSync(filePath);
  if (bytes.length > maxBytes) throw new InteractError(code, message);
  return bytes;
}

async function reconcileImportedAliases(session: InteractSession, aliases: AliasEntry[], entityIdMap: unknown) {
  const remaps = new Map(objectArray(entityIdMap).map((entry) => [Number(entry.oldId), Number(entry.newId)]));
  session.aliases.clear();
  const candidates = aliases.map((entry) => ({ ...entry, id: remaps.get(entry.id) || entry.id }));
  const inspected = candidates.length ? await session.driver.inspect({ ids: candidates.map((entry) => entry.id), limit: candidates.length }) : { entities: [] };
  const existing = new Set(objectArray(inspected.entities).map((entity) => Number(entity.id)));
  const restored: Array<AliasEntry & { oldId?: number }> = [];
  const stale: AliasEntry[] = [];
  for (const entry of candidates) {
    if (existing.has(entry.id)) { session.aliases.set(entry.alias, entry.id); restored.push({ alias: entry.alias, oldId: aliases.find((source) => source.alias === entry.alias)?.id, id: entry.id }); }
    else stale.push({ alias: entry.alias, id: entry.id });
  }
  return { restored, stale };
}

async function validateImportedAliases(session: InteractSession, aliases: AliasEntry[]) {
  session.aliases.clear();
  const inspected = aliases.length ? await session.driver.inspect({ ids: aliases.map((entry) => entry.id), limit: aliases.length }) : { entities: [] };
  const ids = new Set(objectArray(inspected.entities).map((entity) => Number(entity.id)));
  const restored: AliasEntry[] = [], stale: AliasEntry[] = [];
  for (const entry of aliases) {
    if (ids.has(entry.id)) { session.aliases.set(entry.alias, entry.id); restored.push(entry); } else stale.push(entry);
  }
  return { restored, stale };
}

async function validateAliasesAgainstArtifact(_session: InteractSession, kind: string, artifact: JsonObject, aliases: AliasEntry[]) {
  const setup = kind === "setup" ? artifact : asJsonObject(artifact.initialSetup);
  const metadata = asJsonObject(setup.metadata);
  const idMap = objectArray(metadata.sourceEntityIdMap);
  const known = new Set([...artifactEntityIds(setup), ...idMap.flatMap((entry) => [Number(entry.oldId), Number(entry.newId)])]);
  return { entries: aliases, stale: aliases.filter((entry) => !known.has(entry.id)), status: "sidecarValidated" };
}

function artifactSummary(kind: string, artifact: JsonObject, aliases: AliasEntry[]) {
  const setup = kind === "setup" ? artifact : asJsonObject(artifact.initialSetup);
  const timeline = asJsonObject(artifact.timeline);
  const map = asJsonObject(setup.map);
  let entityCount = null;
  try {
    entityCount = artifactEntityIds(setup).length;
  } catch {}
  return {
    authoring: kind === "setup" ? { name: artifact.name || "" } : artifact.authoring || {},
    map: Object.keys(map).length ? { name: map.name, contentHash: map.contentHash, materializedHash: map.materializedHash } : null,
    tick: kind === "setup" ? asJsonObject(setup.metadata).exportedTick ?? null : timeline.initialTick ?? null,
    durationTicks: kind === "replay" ? timeline.durationTicks ?? null : 0,
    entityCount,
    operationCount: kind === "replay" && Array.isArray(artifact.operations) ? artifact.operations.length : 0,
    serverBuildSha: kind === "replay" ? artifact.serverBuildSha || null : null,
    aliasCount: aliases.length,
  };
}

function artifactEntityIds(setup: JsonObject): number[] {
  try {
    const parsed = JSON.parse(typeof setup.checkpointPayload === "string" ? setup.checkpointPayload : "") as unknown;
    const entities = asJsonObject(parsed).entities;
    const rows = Array.isArray(entities) ? entities : asJsonObject(entities).entities;
    return objectArray(rows).map((entity) => Number(entity.id)).filter((id) => Number.isInteger(id) && id > 0);
  } catch { return []; }
}

function reproductionSummary(kind: string | undefined, artifactId: string, aliases: AliasEntry[]) {
  const refs = aliases.slice(0, 12).map((entry) => entry.alias).join(", ");
  const input = JSON.stringify({ sessionId: "<current-session-id>", kind, artifactId });
  return `node scripts/interact/cli.mjs lab import '${input}'; aliases: ${refs || "none"}`;
}

function boundedResponseSummary<T>(values: T[]) {
  const source = Array.isArray(values) ? values : [];
  return {
    count: source.length,
    details: source.slice(0, INTERACT_LIMITS.maxResponseDetails),
    truncated: source.length > INTERACT_LIMITS.maxResponseDetails,
  };
}

function recordingOperation(command: string, input: JsonObject, result: unknown) {
  return {
    command,
    acceptedAt: new Date().toISOString(),
    input: JSON.parse(JSON.stringify(input, (key, value) => key === "sessionId" ? undefined : value)),
    authoritativeTick: findSnapshotTick(result),
  };
}

function findSnapshotTick(value: unknown, depth = 0): number | null {
  if (!isJsonObject(value) || depth > 5) return null;
  if (Number.isInteger(value.snapshotTick)) return Number(value.snapshotTick);
  for (const child of Object.values(value)) {
    const tick = findSnapshotTick(child, depth + 1);
    if (tick != null) return tick;
  }
  return null;
}

function projectCatalog(catalog: JsonObject, requested?: string[]) {
  const categories = [...new Set(requested?.length ? requested : ALL_CATALOG_CATEGORIES)];
  const all = {
    maps: Array.isArray(catalog.maps) ? catalog.maps : [], players: Array.isArray(catalog.players) ? catalog.players : [],
    factions: objectArray(catalog.factions).map((faction) => ({
      id: String(faction.id || ""), label: String(faction.label || ""), units: uniqueStrings(faction.units),
      buildings: uniqueStrings(faction.buildings), upgrades: uniqueStrings(faction.upgrades),
    })),
    units: uniqueStrings(flattenFactions(catalog.factions, "units")), buildings: uniqueStrings(flattenFactions(catalog.factions, "buildings")),
    upgrades: uniqueStrings(flattenFactions(catalog.factions, "upgrades")), commands: uniqueStrings(catalog.supportedCommandKinds), abilities: uniqueStrings(catalog.abilities),
  };
  return { categories: Object.fromEntries(categories.map((category) => [category, all[category as keyof typeof all]])), truncated: false };
}

function flattenFactions(factions: unknown, field: string): string[] { return objectArray(factions).flatMap((faction) => Array.isArray(faction[field]) ? faction[field].filter((value): value is string => typeof value === "string") : []); }
function uniqueStrings(values: unknown): string[] { return [...new Set((Array.isArray(values) ? values : []).filter((value): value is string => typeof value === "string"))].sort(); }

function validateSpawnAliases(session: InteractSession, spawns: SpawnInput[]) {
  const aliases = new Set<string>();
  for (const { alias: value } of spawns) {
    if (!value) continue;
    if (session.aliases.has(value) || aliases.has(value)) throw new InteractError("duplicateAlias", `Alias ${JSON.stringify(value)} is already in use.`);
    aliases.add(value);
  }
  if (session.aliases.size + aliases.size > INTERACT_LIMITS.maxAliases) throw new InteractError("aliasLimit", `At most ${INTERACT_LIMITS.maxAliases} aliases are allowed.`);
}

function assertKnownPlayer(catalog: JsonObject, playerId: unknown) {
  if (playerId == null) return;
  if (!objectArray(catalog.players).some((player) => Number(player.id) === Number(playerId))) throw new InteractError("unknownPlayer", `Player ${playerId} is not available.`);
}

function validateCommandCatalog(command: JsonObject, catalog: JsonObject) {
  if (typeof command.c !== "string") throw new InteractError("invalidCommand", "Command kind is required.");
  if (!new Set(uniqueStrings(catalog.supportedCommandKinds)).has(command.c)) throw new InteractError("unsupportedCommand", `${command.c} is not supported.`);
  const validateFrom = (field: string, values: Iterable<unknown>, code: string, label: string) => {
    if (command[field] != null && !new Set(values).has(command[field])) throw new InteractError(code, `${command[field]} is not an available ${label}.`);
  };
  if (command.c === "build") validateFrom("building", flattenFactions(catalog.factions, "buildings"), "invalidKind", "building");
  validateFrom("unit", flattenFactions(catalog.factions, "units"), "invalidKind", "unit");
  validateFrom("upgrade", flattenFactions(catalog.factions, "upgrades"), "invalidUpgrade", "upgrade");
  validateFrom("ability", uniqueStrings(catalog.abilities), "invalidAbility", "ability");
}

async function resolveCommand(session: InteractSession, command: JsonObject) {
  const wire: JsonObject = { ...command }; const resolved: JsonObject = {};
  for (const field of ["units", "buildings"]) {
    if (!Array.isArray(command[field])) continue;
    const entries = await resolveEntityReferences(session, command[field] as EntityRef[]);
    wire[field] = entries.map((entry) => entry.id);
    resolved[field] = entries;
  }
  for (const field of ["target", "node", "building"]) {
    if (command[field] == null || (field === "building" && command.c === "build")) continue;
    const entry = await resolveEntityReference(session, command[field] as EntityRef); wire[field] = entry.id; resolved[field] = entry;
  }
  return { command: wire, resolved };
}

async function resolveEntityReferences(session: InteractSession, references: EntityRef[]): Promise<ResolvedEntityRef[]> {
  const entries: ResolvedEntityRef[] = []; const requestedIds: number[] = [];
  for (const reference of references) {
    if (typeof reference === "number") { entries.push({ input: reference, id: reference, alias: null }); requestedIds.push(reference); continue; }
    const id = session.aliases.get(reference);
    if (!id) throw new InteractError("unknownAlias", `Unknown alias ${JSON.stringify(reference)}.`);
    entries.push({ input: reference, id, alias: reference }); requestedIds.push(id);
  }
  if (new Set(requestedIds).size !== requestedIds.length) throw new InteractError("duplicateReference", "A command may not resolve the same entity twice.");
  const found = new Set();
  for (let offset = 0; offset < requestedIds.length; offset += INTERACT_LIMITS.maxInspectRefs) {
    const ids = requestedIds.slice(offset, offset + INTERACT_LIMITS.maxInspectRefs);
    const existing = await session.driver.inspect({ ids, limit: ids.length });
    for (const entity of objectArray(existing.entities)) found.add(Number(entity.id));
  }
  for (const entry of entries) {
    if (found.has(entry.id)) continue;
    if (entry.alias) session.aliases.delete(entry.alias);
    throw new InteractError(entry.alias ? "staleAlias" : "unknownEntity", entry.alias ? `Alias ${JSON.stringify(entry.alias)} is stale and was cleared.` : `Entity ${entry.id} is unavailable.`);
  }
  return entries;
}

async function resolveEntityReference(session: InteractSession, reference: EntityRef) { return (await resolveEntityReferences(session, [reference]))[0]; }
async function aliasSnapshots(session: InteractSession) {
  if (!session.aliases.size) return [];
  const entries = [...session.aliases].map(([alias, id]) => ({ alias, id }));
  const inspected = await session.driver.inspect({ ids: entries.map((entry) => entry.id), limit: entries.length });
  const byId = new Map(objectArray(inspected.entities).map((entity) => [Number(entity.id), entity]));
  return entries.map((entry) => ({ ...entry, entity: byId.get(entry.id) || null }));
}
async function reconcileAliasesAfterReset(session: InteractSession, before: Array<AliasEntry & { entity: JsonObject | null }>) {
  session.aliases.clear(); const after = await session.driver.inspect({ limit: INTERACT_LIMITS.maxInspectResults });
  const claimed = new Set(); const aliases = []; const clearedAliases = [];
  for (const entry of before) {
    const matches = objectArray(after.entities).filter((entity) => exactAliasMatch(entry.entity, entity) && !claimed.has(entity.id));
    if (matches.length === 1) { const id = Number(matches[0].id); claimed.add(id); session.aliases.set(entry.alias, id); aliases.push({ alias: entry.alias, id }); }
    else clearedAliases.push(entry.alias);
  }
  return { aliases, clearedAliases };
}
function exactAliasMatch(before: JsonObject | null, after: JsonObject) { return !!before && before.kind === after.kind && before.owner === after.owner && Number(before.x) === Number(after.x) && Number(before.y) === Number(after.y); }
function decorateEntity(entity: unknown, aliases: Map<string, number>) { return isJsonObject(entity) ? { ...entity, alias: aliasForEntity(aliases, entity.id) } : entity || null; }
function aliasForEntity(aliases: Map<string, number>, id: unknown) { for (const [alias, entityId] of aliases) if (entityId === id) return alias; return null; }
function clearAliasesForEntity(aliases: Map<string, number>, id: number) { for (const [alias, entityId] of aliases) if (entityId === id) aliases.delete(alias); }

function isJsonObject(value: unknown): value is JsonObject {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function asJsonObject(value: unknown): JsonObject {
  return isJsonObject(value) ? value : {};
}

function objectArray(value: unknown): JsonObject[] {
  return Array.isArray(value) ? value.filter(isJsonObject) : [];
}
