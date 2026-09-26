/**
 * TypeScript mirror of the Rust IPC contract.
 *
 * These types are hand-maintained against `crates/leanai-core` and
 * `src-tauri/src/commands`. `src/ipc/contract.test.ts` asserts that every
 * command named here exists in the Rust handler list, so a rename on either
 * side fails the test suite rather than the app at runtime.
 */

export type FileClass =
  | "source_text"
  | "binary"
  | "generated"
  | "lockfile"
  | "credential_sensitive"
  | "hidden_metadata"
  | "too_large"
  | "symlink"
  | "unsupported_encoding"
  | "unreadable";

export type IgnoreSource =
  "LeanAiPolicy" | "GitGlobal" | "GitInfoExclude" | "GitIgnore" | "AiIgnore" | "UserOverride";

export interface Exclusion {
  source: IgnoreSource;
  reason: string;
  rule: string | null;
}

export interface FileEntry {
  path: string;
  sizeBytes: number;
  class: FileClass;
  selectable: boolean;
  exclusion: Exclusion | null;
  contentHash: string | null;
  modifiedMs: number | null;
}

export interface ScanIssue {
  path: string;
  message: string;
}

export interface ScanStats {
  filesSeen: number;
  directoriesSeen: number;
  bytesSeen: number;
  elapsedMs: number;
  truncated: boolean;
}

export interface Inventory {
  root: string;
  projectFingerprint: string;
  sourceRevision: string;
  policyVersion: number;
  scannedAtMs: number;
  files: FileEntry[];
  issues: ScanIssue[];
  stats: ScanStats;
}

export interface Limits {
  maxFileBytes: number;
  maxDataFileBytes: number;
  maxFilesScanned: number;
  maxSelectedFiles: number;
  maxBundleBytes: number;
  binaryProbeBytes: number;
  maxDepth: number;
}

export interface Policy {
  version: number;
  limits: Limits;
  respectGitIgnore: boolean;
  respectAiIgnore: boolean;
  includeHidden: boolean;
  followSymlinks: boolean;
  excludeLockfiles: boolean;
  excludeGenerated: boolean;
}

export interface BundleOptions {
  headers: boolean;
  lineNumbers: boolean;
  codeFences: boolean;
  fileSizeAnnotations: boolean;
  includeTree: boolean;
  includeFrontMatter: boolean;
  normalizeLineEndings: boolean;
  maxFileBytes: number | null;
}

export interface SelectionSpec {
  files: string[];
  directories: string[];
  excluded: string[];
  overrides: string[];
}

export interface RejectedPath {
  path: string;
  reason: string;
  overridable: boolean;
}

export interface ResolvedSelection {
  files: string[];
  rejected: RejectedPath[];
  missing: string[];
  totalBytes: number;
}

export type TriState = "unchecked" | "partial" | "checked";

export type SelectionRecipe = "source_only" | "everything" | "tests_only";

export interface SuggestedSelection {
  recipe: SelectionRecipe;
  files: string[];
  totalBytes: number;
  /** Selectable files the recipe deliberately left out. */
  skippedByRecipe: number;
  description: string;
}

export interface DirectorySummary {
  path: string;
  depth: number;
  selectableFiles: number;
  totalBytes: number;
}

export type EstimateKind =
  | { kind: "openai_family_estimate"; tokenizer: string }
  | { kind: "provider_exact"; provider: string; model: string }
  | { kind: "unavailable"; reason: string };

export type TokenEstimate = { value: number } & EstimateKind;

export interface FileContribution {
  path: string;
  bytes: number;
  tokens: number;
  share: number;
}

export interface TruncationWarning {
  path: string;
  originalBytes: number;
  includedBytes: number;
}

export interface BundleManifest {
  format: string;
  projectFingerprint: string;
  sourceRevision: string;
  policyVersion: number;
  createdAtMs: number;
  selection: {
    includedFiles: string[];
    excludedFiles: string[];
    exclusionReasons: Record<string, string>;
  };
  options: BundleOptions;
  tokenEstimate: { value: number; kind: string; tokenizerOrProvider: string };
  warnings: string[];
}

export interface BuildBundleResponse {
  resolved: ResolvedSelection;
  preview: string;
  previewTruncated: boolean;
  outputHash: string;
  estimate: TokenEstimate;
  estimateLabel: string;
  contributions: FileContribution[];
  truncations: TruncationWarning[];
  skipped: string[];
  byteLen: number;
  fileCount: number;
  manifest: BundleManifest;
}

export type Confidence = "high" | "medium" | "low";

export interface SecretFinding {
  path: string;
  line: number;
  rule: string;
  confidence: Confidence;
  redactedExcerpt: string;
  dismissed: boolean;
}

export interface SecretReport {
  findings: SecretFinding[];
  high: number;
  medium: number;
  low: number;
  skipped: string[];
}

export type ExportDestination = "clipboard" | "file";

export interface ExportPreflight {
  fileCount: number;
  byteLen: number;
  estimate: TokenEstimate;
  estimateLabel: string;
  includedFiles: string[];
  secretReport: SecretReport;
  requiresConfirmation: boolean;
  disclaimer: string;
  destinationNote: string;
}

export interface ExportResponse {
  text: string | null;
  writtenPath: string | null;
  outputHash: string;
  bundleId: string | null;
  manifestPath: string | null;
}

export interface ProjectRecord {
  id: string;
  fingerprint: string;
  canonicalPath: string;
  displayName: string;
  lastScanRevision: string | null;
  lastOpenedAtMs: number | null;
  policyVersion: number;
}

export interface GitState {
  isRepository: boolean;
  headRef: string | null;
  headCommit: string | null;
  isDirty: boolean;
}

export type DiffScope = "staged" | "unstaged" | "untracked" | "against_ref";

export interface ChangedFile {
  path: string;
  status: string;
  scope: DiffScope;
}

export interface OpenProjectResponse {
  project: ProjectRecord;
  git: GitState;
  hasAiIgnore: boolean;
}

export interface ScanResponse {
  inventory: Inventory;
  classCounts: Record<string, number>;
  selectableBytes: number;
}

export interface AiIgnorePreview {
  newlyExcluded: string[];
  unmatchedRules: string[];
  invalidRules: string[];
  selectableBefore: number;
  selectableAfter: number;
}

export interface AiIgnoreFile {
  exists: boolean;
  contents: string;
  template: string;
}

export interface PresetRecord {
  id: string;
  projectId: string;
  name: string;
  selection: SelectionSpec;
  options: BundleOptions;
  updatedAtMs: number;
  validation: ResolvedSelection | null;
}

export interface BundleRecord {
  id: string;
  projectId: string;
  sourceRevision: string;
  outputHash: string;
  estimateValue: number;
  estimateKind: string;
  fileCount: number;
  byteLen: number;
  hasText: boolean;
  createdAtMs: number;
}

export type Freshness = "fresh" | "stale" | "unknown";

export interface SourceRef {
  path: string;
  contentHash: string | null;
  lines: [number, number] | null;
}

export type Generator =
  | { generator: "deterministic" }
  | { generator: "model"; provider: string; model: string; promptVersion: string };

export interface ContextSection {
  key: string;
  title: string;
  body: string;
  sourceRefs: SourceRef[];
  freshness: Freshness;
  generator: Generator;
  limitations: string[];
  generatedRevision: string;
  generatedAtMs: number;
}

export interface ContextDocument {
  schemaVersion: number;
  projectFingerprint: string;
  sourceRevision: string;
  generatedAtMs: number;
  contentHash: string;
  sections: ContextSection[];
}

export interface ContextResponse {
  document: ContextDocument;
  markdown: string;
  freshness: Freshness;
  staleSectionKeys: string[];
}

export interface ChangeImpact {
  changedFiles: string[];
  deletedFiles: string[];
  addedFiles: string[];
  staleSections: string[];
  unverifiableSections: string[];
}

export interface SourceOnDemandResponse {
  path: string;
  content: string;
  contentHash: string;
  changedSinceScan: boolean;
  fromLine: number;
  toLine: number;
  totalLines: number;
}

export type BundleRetention = "metadata_only" | "full_text" | "none";

export interface Settings {
  schema: number;
  policy: Policy;
  defaultBundleOptions: BundleOptions;
  bundleRetention: BundleRetention;
  maxHistoryEntries: number;
  telemetryOptIn: boolean;
  requireExportConfirmation: boolean;
}

export interface PolicyDescription {
  policy_version: number;
  limits: Limits;
  limit_reasons: { key: string; reason: string }[];
  ignore_precedence: { source: IgnoreSource; label: string; rank: number }[];
  secret_scan_disclaimer: string;
  always_skipped_directories: string[];
}

export interface Diagnostics {
  appVersion: string;
  schemaVersion: number;
  latestSchemaVersion: number;
  policyVersion: number;
  platform: string;
  arch: string;
  projectOpen: boolean;
  telemetryOptIn: boolean;
  capabilities: string[];
  notice: string;
}

export interface AuditEntry {
  id: string;
  task: string;
  mode: string;
  manifest: unknown;
  status: string;
  startedAtMs: number;
}

/** The structured error every command returns on failure. */
export interface AppError {
  code: string;
  message: string;
  recovery: string | null;
  retryable: boolean;
}

export interface ScanProgress {
  phase: string;
  filesSeen: number;
  directoriesSeen: number;
  bytesSeen: number;
}

export const SCAN_PROGRESS_EVENT = "leanai://scan-progress";

export type CachedTokenPolicy = "none" | "prompt_prefix" | "automatic";

export interface CapabilityProfile {
  contextCap: number;
  streaming: boolean;
  toolCalling: boolean;
  structuredOutput: boolean;
  vision: boolean;
  exactTokenCounting: boolean;
  cachedTokenPolicy: CachedTokenPolicy;
}

export interface ModelRecord {
  id: string;
  kind: string;
  displayName: string;
  source: string;
  version: string;
  filePath: string | null;
  capabilityProfile: CapabilityProfile;
  checksum: string | null;
  status: string;
  createdAtMs: number;
  updatedAtMs: number;
}

export interface ProviderConfig {
  providerId: string;
  displayName: string;
  accountLabel: string;
  isConfigured: boolean;
  priceCatalogVer: string;
  enabled: boolean;
  updatedAtMs: number;
}

export type SidecarStatus =
  | { state: "stopped" }
  | { state: "starting"; port: number; modelId: string }
  | { state: "ready"; port: number; pid: number; modelId: string; displayName: string }
  | { state: "stopping" }
  | { state: "error"; message: string };

export interface ModelPrice {
  inputUsdPer1m?: number;
  outputUsdPer1m?: number;
  cachedInputUsdPer1m?: number;
  inputUsdPer1M?: number;
  outputUsdPer1M?: number;
  cachedInputUsdPer1M?: number;
  currency?: string;
}

export interface CatalogEntry {
  modelId: string;
  displayName: string;
  provider: string;
  contextCap: number;
  pricing: ModelPrice;
  capabilities: CapabilityProfile;
}

export interface PriceCatalog {
  version: string;
  updatedAtMs: number;
  updateSource: string;
  entries: CatalogEntry[];
}

export type TaskClass =
  | "file_inventory"
  | "context_documentation"
  | "multi_file_planning"
  | "code_change_proposal"
  | "mechanical_validation";

export type RoutingReason =
  | "task_default_low_cost"
  | "task_requires_strong_reasoning"
  | "escalated_context_exceeded_low_cost_cap"
  | "local_only_enforced";

export interface RoutingDecision {
  taskClass: TaskClass;
  selectedModelId: string;
  selectedProvider: string;
  reason: RoutingReason;
  explanation: string;
  estimatedCostUsd: number | null;
}

export interface TokenCountResult {
  count: number;
  estimateKind: { [key: string]: unknown } | string;
}

export interface ProviderStatusResponse {
  providerId: string;
  isConfigured: boolean;
  isStaleCatalog: boolean;
}

export type AgentRole =
  "orchestrator" | "planner" | "context_builder" | "coder" | "validator" | "tester";

export type ToolCapability =
  "read_file" | "inspect_context_index" | "propose_patch" | "write_file" | "execute_command";

export interface Subtask {
  id: string;
  title: string;
  description: string;
  targetFiles: string[];
  expectedOutcome: string;
}

export interface PlanArtifact {
  objective: string;
  subtasks: Subtask[];
  definitionOfSuccess: string;
}

export interface ContextCitation {
  relativePath: string;
  lineRange?: [number, number];
  sha256: string;
  estimatedTokens: number;
}

export interface ContextBuilderArtifact {
  citations: ContextCitation[];
  omittedContextReasons: Record<string, string>;
  totalEstimatedTokens: number;
}

export interface FilePatch {
  path: string;
  unifiedDiff: string;
  isNewFile: boolean;
  isDeleted: boolean;
  linesAdded: number;
  linesDeleted: number;
}

export interface PatchProposal {
  summary: string;
  rationale: string;
  affectedFiles: string[];
  patches: FilePatch[];
  sha256Hash: string;
}

export interface ValidationCheck {
  checkName: string;
  passed: boolean;
  message: string;
  details?: string;
}

export interface ValidatorVerdict {
  isValid: boolean;
  checks: ValidationCheck[];
  blockerSummary?: string;
}

export type DecisionState = "pending" | "approved" | "denied" | "timed_out";

export interface ApprovalRequest {
  approvalId: string;
  runId: string;
  requestedCapability: ToolCapability;
  projectRoot: string;
  targetPaths: string[];
  diffHash: string;
  state: DecisionState;
  approver?: string;
  requestedAtMs: number;
  decidedAtMs?: number;
  ttlMs: number;
}

export interface StepRecord {
  id: string;
  role: AgentRole;
  action: string;
  status: string;
  tokensUsed: number;
  timestampMs: number;
  details: string;
  evidence?: string;
}

export interface TesterArtifact {
  command: string;
  passed: boolean;
  exitCode?: number;
  stdout: string;
  stderr: string;
  durationMs: number;
  timestampMs: number;
  summary: string;
}

export interface RunTesterRequest {
  runId?: string;
  command: string;
}

export interface TaskRunResponse {
  runId: string;
  status: string;
  plan: PlanArtifact;
  contextManifest: ContextBuilderArtifact;
  patchProposal?: PatchProposal;
  validatorVerdict: ValidatorVerdict;
  testerVerdict?: TesterArtifact;
  pendingApproval?: ApprovalRequest;
  steps: StepRecord[];
  startedAtMs: number;
  endedAtMs?: number;
}

export interface StartTaskRequest {
  objective: string;
  contextFiles: string[];
  providerModelId?: string;
  testCommand?: string;
  maxTokens?: number;
  maxCostUsd?: number;
}

export interface ResolveApprovalRequest {
  approvalId: string;
  approved: boolean;
  approver?: string;
  proposal?: PatchProposal;
}

export interface ResolveApprovalResponse {
  approvalId: string;
  decision: string;
  patchApplied: boolean;
  rollbackPerformed: boolean;
  message: string;
}

export interface RunRecord {
  id: string;
  projectId: string;
  task: string;
  mode: string;
  contextManifest: unknown;
  policy: unknown;
  status: string;
  budget?: unknown;
  startedAtMs: number;
  endedAtMs?: number;
  validationState?: unknown;
}

export interface TaskDetailResponse {
  run: RunRecord;
  events: unknown[];
}

export interface ProposedFile {
  path: string;
  score: number;
  reasons: string[];
  sizeBytes: number;
  estimatedTokens: number;
}

export interface EpisodicMemoryEntry {
  id: string;
  taskSummary: string;
  relevantPaths: string[];
  keyFindings: string;
  createdAtMs: number;
  expiresAtMs: number;
}

export interface RetrievalResult {
  query: string;
  proposedFiles: ProposedFile[];
  episodicMemories: EpisodicMemoryEntry[];
  totalEstimatedTokens: number;
  rankingDisclaimer: string;
}

export interface QueryTaskContextRequest {
  query: string;
  pinnedPaths?: string[];
  enabled?: boolean;
}

export interface GitHubRepository {
  id: number;
  name: string;
  fullName: string;
  owner: string;
  isPrivate: boolean;
  isFork: boolean;
  htmlUrl: string;
  cloneUrl: string;
  sshUrl: string;
  description: string | null;
  language: string | null;
  stars: number;
  defaultBranch: string;
  updatedAt: string;
}

export interface GitHubUserProfile {
  login: string;
  name: string | null;
  avatarUrl: string | null;
  htmlUrl: string | null;
  scopes: string[];
}

export interface SshKeyInfo {
  filename: string;
  keyType: string;
  comment: string;
  publicKey: string;
}

export interface GitAuthStatus {
  githubTokenConfigured: boolean;
  githubUser: GitHubUserProfile | null;
  sshAgentActive: boolean;
  sshKeys: SshKeyInfo[];
  sshAuthenticated: boolean;
  sshUsername: string | null;
}

export interface ConfigureGithubTokenRequest {
  token: string;
  accountLabel?: string;
}

export interface SshAuthTestResult {
  authenticated: boolean;
  username: string | null;
  output: string;
}

export interface RemoteInfo {
  name: string;
  url: string;
  protocol: string;
}

export interface CommitSummary {
  id: string;
  summary: string;
  author: string;
  timestampMs: number;
}

export interface GitRemoteStatus {
  isRepository: boolean;
  currentBranch: string | null;
  remotes: RemoteInfo[];
  upstreamBranch: string | null;
  ahead: number;
  behind: number;
  unpushedCommits: CommitSummary[];
  isDirty: boolean;
}

export interface GitPushRequest {
  remote?: string;
  branch?: string;
  force?: boolean;
}

export interface GitPushResponse {
  success: boolean;
  message: string;
  remote: string;
  branch: string;
}

export interface CloneRepositoryRequest {
  url: string;
  destinationParentDir: string;
  directoryName?: string;
}
