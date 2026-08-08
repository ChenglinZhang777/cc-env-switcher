import React, { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import "./styles.css";

type Provider = { id: string; name: string; env: Record<string, string> };
const emptyProvider = (): Provider => ({ id: crypto.randomUUID(), name: "新供应商", env: { ANTHROPIC_BASE_URL: "", ANTHROPIC_AUTH_TOKEN: "" } });

function App() {
  const [providers, setProviders] = useState<Provider[]>([]); const [selected, setSelected] = useState<Provider | null>(null); const [message, setMessage] = useState("");
  const load = async () => { const items = await invoke<Provider[]>("list_providers"); setProviders(items); if (!selected && items[0]) setSelected(items[0]); };
  useEffect(() => { void load().catch(error => setMessage(String(error))); }, []);
  const save = async () => { if (!selected?.name.trim()) return setMessage("请填写供应商名称。"); await invoke("save_provider", { profile: selected }); await load(); setMessage("供应商已保存。密钥会以明文保存在本机应用配置中。"); };
  const remove = async () => { if (!selected) return; await invoke("delete_provider", { id: selected.id }); setSelected(null); await load(); };
  const switchTo = async (provider: Provider) => { try { await invoke("switch_provider", { id: provider.id }); setMessage(`已切换到 ${provider.name}，并已先创建完整备份。`); } catch (error) { setMessage(`切换失败：${String(error)}`); } };
  const importCurrent = async () => { try { const env = await invoke<Record<string, string>>("import_current_env"); setSelected({ id: crypto.randomUUID(), name: "从当前配置导入", env }); setMessage("已导入当前 env，请命名后保存。"); } catch (error) { setMessage(`导入失败：${String(error)}`); } };
  const changeEnv = (key: string, value: string) => selected && setSelected({ ...selected, env: { ...selected.env, [key]: value } });
  return <main><header><div><h1>Claude Env Switcher</h1><p>只切换 ~/.claude/settings.json 的 env；每次切换都完整备份。</p></div><div className="actions"><button onClick={() => void invoke("open_backups_directory")}>打开备份目录</button><button onClick={() => { const item = emptyProvider(); setProviders([...providers, item]); setSelected(item); }}>新增供应商</button></div></header><section className="layout"><aside><h2>供应商</h2>{providers.map(item => <div className="provider" key={item.id}><button className={selected?.id === item.id ? "selected" : ""} onClick={() => setSelected(item)}>{item.name}</button><button className="switch" onClick={() => void switchTo(item)}>切换</button></div>)}</aside><article>{selected ? <><label>名称<input value={selected.name} onChange={event => setSelected({ ...selected, name: event.target.value })} /></label><h2>环境变量</h2>{Object.entries(selected.env).map(([key, value]) => <div className="env" key={key}><input value={key} onChange={event => { const next = { ...selected.env }; delete next[key]; next[event.target.value] = value; setSelected({ ...selected, env: next }); }} /><input value={value} type="password" onChange={event => changeEnv(key, event.target.value)} /><button onClick={() => { const next = { ...selected.env }; delete next[key]; setSelected({ ...selected, env: next }); }}>删除</button></div>)}<button onClick={() => changeEnv("", "")}>添加变量</button><footer><button onClick={() => void save()}>保存方案</button><button onClick={() => void importCurrent()}>从当前配置导入</button><button className="danger" onClick={() => void remove()}>删除方案</button></footer></> : <p>选择或新增一个供应商。</p>}</article></section>{message && <output>{message}</output>}</main>;
}
createRoot(document.getElementById("root")!).render(<App />);
