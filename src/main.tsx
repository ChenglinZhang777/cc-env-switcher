import React, { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import { defaultProviderEnv } from "./providerTemplate";
import { missingConnectionFields, presentConnectionResult, type ConnectionResultKind } from "./connectionTest";
import { checkForUpdate, installUpdate, presentUpdateResult, type AvailableUpdate } from "./update";
import { detectActiveState, presentActiveBadge, type ActiveState } from "./activeProvider";
import { errorFeedback, FEEDBACK_DISMISS_MS, successFeedback, type Feedback } from "./feedback";
import "./styles.css";

type Provider = { id: string; name: string; env: Record<string, string> };
const modelKeys = ["ANTHROPIC_DEFAULT_OPUS_MODEL", "ANTHROPIC_DEFAULT_SONNET_MODEL", "ANTHROPIC_DEFAULT_HAIKU_MODEL", "ANTHROPIC_DEFAULT_FABLE_MODEL"] as const;
const labels: Record<(typeof modelKeys)[number], string> = { ANTHROPIC_DEFAULT_OPUS_MODEL: "Opus", ANTHROPIC_DEFAULT_SONNET_MODEL: "Sonnet", ANTHROPIC_DEFAULT_HAIKU_MODEL: "Haiku", ANTHROPIC_DEFAULT_FABLE_MODEL: "Fable" };
const newProvider = (): Provider => ({ id: crypto.randomUUID(), name: "新供应商", env: { ...defaultProviderEnv } });
const host = (value = "") => { try { return value ? new URL(value).host : "未设置地址"; } catch { return value; } };

function App() {
  const [providers, setProviders] = useState<Provider[]>([]);
  const [selected, setSelected] = useState<Provider | null>(null);
  const [feedback, setFeedback] = useState<Feedback | null>(null);
  const [busyAction, setBusyAction] = useState<"" | "saved" | "switched">("");
  const [activeState, setActiveState] = useState<ActiveState>({ kind: "unreadable" });
  const [showToken, setShowToken] = useState(false);
  const [testingConnection, setTestingConnection] = useState(false);
  const [connectionMessage, setConnectionMessage] = useState("");
  const [availableUpdate, setAvailableUpdate] = useState<AvailableUpdate | null>(null);
  const [updateMessage, setUpdateMessage] = useState("");
  const [installing, setInstalling] = useState(false);

  const notify = (next: Feedback) => {
    setFeedback(next);
    if (!next.sticky) setTimeout(() => setFeedback(current => (current === next ? null : current)), FEEDBACK_DISMISS_MS);
  };
  const confirmAction = (action: "saved" | "switched", text: string) => {
    setBusyAction(action);
    notify(successFeedback(text));
    setTimeout(() => setBusyAction(current => (current === action ? "" : current)), FEEDBACK_DISMISS_MS);
  };
  const refreshActiveState = async (profiles: Provider[]) => {
    const activeEnv = await invoke<Record<string, string> | null>("read_active_env").catch(() => null);
    setActiveState(detectActiveState(activeEnv, profiles));
  };

  const load = async () => {
    const profiles = await invoke<Provider[]>("list_providers");
    setProviders(profiles);
    setSelected(current => current ? profiles.find(item => item.id === current.id) ?? current : profiles[0] ?? null);
    await refreshActiveState(profiles);
  };
  const checkUpdates = async () => {
    try {
      const update = await checkForUpdate();
      if (update) {
        setAvailableUpdate(update);
        setUpdateMessage(presentUpdateResult({ kind: "available", version: update.version, notes: update.notes }).title);
      } else {
        setUpdateMessage(presentUpdateResult({ kind: "current" }).title);
      }
    } catch {
      setUpdateMessage(presentUpdateResult({ kind: "failed" }).detail);
    }
  };
  useEffect(() => { void load().catch(error => notify(errorFeedback(String(error)))); void checkUpdates(); }, []);

  const setEnv = (key: string, value: string) => selected && setSelected({ ...selected, env: { ...selected.env, [key]: value } });
  const save = async () => {
    if (!selected?.name.trim()) return notify(errorFeedback("请先填写供应商名称。"));
    try { await invoke("save_provider", { profile: selected }); await load(); confirmAction("saved", "方案已保存。密钥以明文保存在本机应用配置中。"); }
    catch (error) { notify(errorFeedback(`保存失败：${String(error)}`)); }
  };
  const testConnection = async () => {
    if (!selected) return;
    const missing = missingConnectionFields(selected.env);
    if (missing.length) { setConnectionMessage(`请先填写：${missing.join("、")}。`); return; }
    setTestingConnection(true); setConnectionMessage("");
    try {
      const result = await invoke<{ kind: ConnectionResultKind }>("test_connection", { input: { baseUrl: selected.env.ANTHROPIC_BASE_URL, authToken: selected.env.ANTHROPIC_AUTH_TOKEN, model: selected.env.ANTHROPIC_MODEL } });
      setConnectionMessage(presentConnectionResult(result.kind));
    } catch {
      setConnectionMessage(presentConnectionResult("network"));
    } finally { setTestingConnection(false); }
  };
  const switchTo = async (provider: Provider) => {
    try { await invoke("switch_provider", { id: provider.id }); await load(); confirmAction("switched", `已切换到 ${provider.name}；原 settings.json 已完整备份。`); }
    catch (error) { notify(errorFeedback(`切换失败：${String(error)}`)); }
  };
  const remove = async () => {
    if (!selected || !confirm(`删除"${selected.name}"方案？不会删除 Claude 配置或任何备份。`)) return;
    try { await invoke("delete_provider", { id: selected.id }); setSelected(null); await load(); notify(successFeedback("方案已删除。")); }
    catch (error) { notify(errorFeedback(`删除失败：${String(error)}`)); }
  };
  const importCurrent = async () => {
    try {
      const env = await invoke<Record<string, string>>("import_current_env");
      setSelected({ id: crypto.randomUUID(), name: "从当前配置导入", env: { ...defaultProviderEnv, ...env } });
      notify(successFeedback("已导入当前 env；请命名后保存。"));
    } catch (error) { notify(errorFeedback(`导入失败：${String(error)}`)); }
  };
  const syncModels = () => {
    if (!selected) return;
    const model = selected.env.ANTHROPIC_MODEL;
    setSelected({ ...selected, env: { ...selected.env, CLAUDE_CODE_SUBAGENT_MODEL: model, ...Object.fromEntries(modelKeys.map(key => [key, model])) } });
  };
  const applyUpdate = async () => {
    if (!availableUpdate) return;
    setInstalling(true);
    try { await installUpdate(availableUpdate.update, () => setUpdateMessage("正在下载并验证更新…")); }
    catch { setUpdateMessage("更新安装失败；当前版本仍可正常使用。"); setInstalling(false); }
  };
  const exportToFile = async () => {
    try {
      const exported = await invoke<boolean>("export_profiles_to_file");
      if (exported) notify(successFeedback("配置已导出到文件。"));
    } catch (error) { notify(errorFeedback(`导出失败：${String(error)}`)); }
  };
  const exportToClipboard = async () => {
    try {
      const exported = await invoke<boolean>("export_profiles_to_clipboard");
      if (exported) notify(successFeedback("配置已复制到剪贴板。"));
    } catch (error) { notify(errorFeedback(`导出失败：${String(error)}`)); }
  };
  const importFromFile = async () => {
    try {
      const result = await invoke<{ imported: number; renamed: number } | null>("import_profiles_from_file");
      if (result) {
        await load();
        notify(successFeedback(`已导入 ${result.imported} 个方案${result.renamed > 0 ? `（${result.renamed} 个重名方案已自动改名）` : ""}。`));
      }
    } catch (error) { notify(errorFeedback(`导入失败：${String(error)}`)); }
  };
  const importFromClipboard = async () => {
    try {
      const result = await invoke<{ imported: number; renamed: number }>("import_profiles_from_clipboard");
      await load();
      notify(successFeedback(`已导入 ${result.imported} 个方案${result.renamed > 0 ? `（${result.renamed} 个重名方案已自动改名）` : ""}。`));
    } catch (error) { notify(errorFeedback(`导入失败：${String(error)}`)); }
  };

  return <main className="app-shell">
    <header className="topbar">
      <div><span className="eyebrow">CLAUDE CODE · PROVIDER CONTROL</span><h1>CC Env Switcher</h1><p>安全切换供应商，每次写入前自动备份。</p></div>
      <div className="header-actions">
        <button className="secondary" onClick={() => void checkUpdates()}>检查更新</button>
        <button className="secondary" onClick={() => void invoke("open_backups_directory")}>查看备份</button>
        <div className="button-group">
          <button className="secondary" onClick={() => void importFromFile()}>从文件导入</button>
          <button className="secondary" onClick={() => void importFromClipboard()}>从剪贴板导入</button>
        </div>
        <div className="button-group">
          <button className="secondary" onClick={() => void exportToFile()}>导出到文件</button>
          <button className="secondary" onClick={() => void exportToClipboard()}>导出到剪贴板</button>
        </div>
        <button className="primary" onClick={() => { const item = newProvider(); setProviders([...providers, item]); setSelected(item); }}>＋ 新增方案</button>
      </div>
    </header>
    {availableUpdate && <section className="update-banner"><div><strong>发现新版本 {availableUpdate.version}</strong><p>{availableUpdate.notes || "已准备好安装最新版本。"}</p></div><div><button className="secondary" onClick={() => setAvailableUpdate(null)}>稍后</button><button className="primary" disabled={installing} onClick={() => void applyUpdate()}>{installing ? "正在安装…" : "立即安装"}</button></div></section>}
    <section className="workspace">
      <aside className="sidebar">
        <div className="sidebar-title"><h2>供应商方案</h2><span>{providers.length}</span></div>
        {activeState.kind === "unknown" && <div className="active-unknown">当前生效的配置不属于任何已存方案。</div>}
        {providers.length ? providers.map(item => { const badge = presentActiveBadge(activeState, item.id); return <button className={`profile-card ${selected?.id === item.id ? "active" : ""}`} key={item.id} onClick={() => setSelected(item)}><span className="profile-name">{item.name}{badge && <em className={`badge ${badge === "已生效" ? "badge-active" : "badge-stale"}`}>{badge}</em>}</span><span>{host(item.env.ANTHROPIC_BASE_URL)}</span><span className="profile-model">{item.env.ANTHROPIC_MODEL || "未设置模型"}</span></button>; }) : <div className="empty-list">还没有方案<br />从当前 Claude 配置导入，或创建新的方案。</div>}
      </aside>
      <article className="editor">
        {selected ? <>
          <div className="editor-heading"><div><span className="eyebrow">正在编辑</span><h2>{selected.name || "未命名方案"}</h2></div><button className="switch-button" disabled={busyAction === "switched"} onClick={() => void switchTo(selected)}>{busyAction === "switched" ? "✓ 已切换" : "切换到此方案 →"}</button></div>
          <section className="form-card"><div className="section-title"><div><h3>连接与主模型</h3><p>这是每次切换写入 Claude Code 的核心环境变量。</p></div></div>
            <label className="field full"><span>方案名称</span><input value={selected.name} placeholder="例如：DeepSeek V4 Flash" onChange={event => setSelected({ ...selected, name: event.target.value })} /></label>
            <div className="field-grid"><label className="field"><span>API 地址</span><input value={selected.env.ANTHROPIC_BASE_URL} placeholder="https://api.example.com/anthropic" onChange={event => setEnv("ANTHROPIC_BASE_URL", event.target.value)} /></label><label className="field"><span>主模型</span><input value={selected.env.ANTHROPIC_MODEL} placeholder="deepseek-v4-flash[1m]" onChange={event => setEnv("ANTHROPIC_MODEL", event.target.value)} /></label></div>
            <label className="field token-field"><span>API Key</span><div><input type={showToken ? "text" : "password"} value={selected.env.ANTHROPIC_AUTH_TOKEN} placeholder="sk-…" onChange={event => setEnv("ANTHROPIC_AUTH_TOKEN", event.target.value)} /><button onClick={() => setShowToken(!showToken)}>{showToken ? "隐藏" : "显示"}</button></div></label>
            <div className="connection-test-row"><button className="secondary" disabled={testingConnection} onClick={() => void testConnection()}>{testingConnection ? "正在测试…" : "测试连接"}</button>{connectionMessage && <span className="connection-result">{connectionMessage}</span>}</div>
          </section>
          <section className="form-card"><div className="section-title"><div><h3>模型映射</h3><p>默认让所有 Claude 模型角色使用同一主模型；可以单独覆写。</p></div><button className="secondary" onClick={syncModels}>同步主模型</button></div><div className="model-grid">{modelKeys.map(key => <label className="field" key={key}><span>{labels[key]}</span><input value={selected.env[key]} placeholder="跟随主模型" onChange={event => setEnv(key, event.target.value)} /></label>)}</div></section>
          <section className="form-card compact"><div className="section-title"><div><h3>Agent 行为</h3><p>针对 Claude Code 的运行设置。</p></div></div><div className="field-grid"><label className="field"><span>Subagent 模型</span><input value={selected.env.CLAUDE_CODE_SUBAGENT_MODEL} placeholder="跟随主模型" onChange={event => setEnv("CLAUDE_CODE_SUBAGENT_MODEL", event.target.value)} /></label><label className="field"><span>工作强度</span><select value={selected.env.CLAUDE_CODE_EFFORT_LEVEL} onChange={event => setEnv("CLAUDE_CODE_EFFORT_LEVEL", event.target.value)}><option value="low">低</option><option value="medium">中</option><option value="high">高</option><option value="max">最高</option></select></label></div><label className="toggle"><input type="checkbox" checked={selected.env.CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS === "1"} onChange={event => setEnv("CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS", event.target.checked ? "1" : "0")} /><span><strong>启用实验性 Agent Teams</strong><small>写入 CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1</small></span></label><label className="toggle"><input type="checkbox" checked={selected.env.CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC === "1"} onChange={event => setEnv("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", event.target.checked ? "1" : "0")} /><span><strong>关闭非必要网络流量</strong><small>写入 CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1</small></span></label><label className="toggle"><input type="checkbox" checked={selected.env.CLAUDE_CODE_ATTRIBUTION_HEADER === "0"} onChange={event => setEnv("CLAUDE_CODE_ATTRIBUTION_HEADER", event.target.checked ? "0" : "1")} /><span><strong>关闭 Attribution Header</strong><small>写入 CLAUDE_CODE_ATTRIBUTION_HEADER=0</small></span></label></section>
          <footer className="editor-footer"><button className="danger" onClick={() => void remove()}>删除方案</button><div><button className="secondary" onClick={() => void importCurrent()}>从当前配置导入</button><button className="primary" disabled={busyAction === "saved"} onClick={() => void save()}>{busyAction === "saved" ? "✓ 已保存" : "保存方案"}</button></div></footer>
        </> : <div className="empty-editor"><h2>先选择一个供应商方案</h2><p>可以新建方案，或从当前 settings.json 导入现有 env。</p><button className="primary" onClick={() => void importCurrent()}>从当前配置导入</button></div>}
      </article>
    </section>
    {updateMessage && <output className="notice">{updateMessage}</output>}
    {feedback && <output className={`toast toast-${feedback.tone}`}>{feedback.text}{feedback.sticky && <button onClick={() => setFeedback(null)}>关闭</button>}</output>}
  </main>;
}
createRoot(document.getElementById("root")!).render(<App />);
