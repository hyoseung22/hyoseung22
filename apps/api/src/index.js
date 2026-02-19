import express from 'express';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { randomUUID } from 'node:crypto';
import { DatabaseSync } from 'node:sqlite';
import fs from 'node:fs';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.resolve(__dirname, '../../..');
const webDir = path.resolve(rootDir, 'apps/web');
const dbPath = path.resolve(rootDir, 'orchestrator.sqlite');
const db = new DatabaseSync(dbPath);
db.exec('PRAGMA foreign_keys = ON');
db.exec(fs.readFileSync(path.resolve(rootDir, 'docs/schema.sql'), 'utf8'));

const deptCount = db.prepare('SELECT COUNT(*) c FROM departments').get().c;
if (!deptCount) {
  const ins = db.prepare('INSERT INTO departments (id,name,icon,color,sort_order) VALUES (?,?,?,?,?)');
  [['planning','Planning','📋','#0ea5e9',1],['gameplay_dev','Gameplay Dev','🎮','#3b82f6',2],['outgame_dev','Outgame Dev','🧩','#6366f1',3],['art','Art','🎨','#ec4899',4],['uiux','UI/UX','🖼️','#f59e0b',5],['qa','QA','✅','#10b981',6],['balance','Balance','⚖️','#f97316',7],['data','Data','📊','#8b5cf6',8],['pm','PM','🧭','#14b8a6',9]].forEach(r=>ins.run(...r));
}
if (!db.prepare('SELECT COUNT(*) c FROM users').get().c) db.prepare('INSERT INTO users (id,email,display_name,role,password_hash) VALUES (?,?,?,?,?)').run(randomUUID(),'owner@local','Owner','owner','dev-only');
if (!db.prepare('SELECT COUNT(*) c FROM agents').get().c) {
  const ins = db.prepare('INSERT INTO agents (id,name,role_level,department_id,provider,specialties_json,status,active) VALUES (?,?,?,?,?,?,?,?)');
  ins.run(randomUUID(),'Planner One','lead','planning','claude',JSON.stringify(['requirements']),'idle',1);
  ins.run(randomUUID(),'Gameplay Dev One','senior','gameplay_dev','codex',JSON.stringify(['combat']),'idle',1);
}

const app = express();
app.use(express.json());
app.use(express.static(webDir));

app.get('/api/health', (_q, s) => s.json({ ok: true, dbPath }));
app.get('/api/departments', (_q,s)=>s.json({ departments: db.prepare('SELECT * FROM departments ORDER BY sort_order').all() }));
app.get('/api/agents', (_q,s)=>s.json({ agents: db.prepare('SELECT * FROM agents ORDER BY created_at DESC').all().map(a=>({...a,specialties:JSON.parse(a.specialties_json||'[]')})) }));
app.post('/api/agents', (q,s)=>{const {name,role_level,department_id,provider,specialties=[]}=q.body??{}; if(!name||!role_level||!department_id||!provider) return s.status(400).json({error:'required fields missing'}); const id=randomUUID(); db.prepare('INSERT INTO agents (id,name,role_level,department_id,provider,specialties_json,status,active) VALUES (?,?,?,?,?,?,?,?)').run(id,name,role_level,department_id,provider,JSON.stringify(specialties),'idle',1); db.prepare('INSERT INTO audit_logs (id,actor_role,action,target_type,target_id,after_json) VALUES (?,?,?,?,?,?)').run(randomUUID(),'manager','AGENT_CREATE','agent',id,JSON.stringify(q.body)); s.status(201).json({id});});
app.delete('/api/agents/:id',(q,s)=>{db.prepare('DELETE FROM agents WHERE id=?').run(q.params.id); s.status(204).end();});
app.get('/api/tasks',(q,s)=>{const t=q.query.phase?db.prepare('SELECT * FROM tasks WHERE phase=? ORDER BY updated_at DESC').all(String(q.query.phase)):db.prepare('SELECT * FROM tasks ORDER BY updated_at DESC').all(); s.json({tasks:t});});
app.post('/api/tasks',(q,s)=>{const {title,description=null,department_id=null,priority=3,acceptance_criteria=null}=q.body??{}; if(!title) return s.status(400).json({error:'title is required'}); const id=randomUUID(); db.prepare("INSERT INTO tasks (id,title,description,department_id,phase,priority,acceptance_criteria,created_at,updated_at) VALUES (?,?,?,?, 'needs',?,?,unixepoch(),unixepoch())").run(id,title,description,department_id,priority,acceptance_criteria); s.status(201).json({id});});
app.post('/api/tasks/:id/assign',(q,s)=>{if(!q.body?.agent_id) return s.status(400).json({error:'agent_id is required'}); db.prepare('UPDATE tasks SET assigned_agent_id=?,updated_at=unixepoch() WHERE id=?').run(q.body.agent_id,q.params.id); s.json({ok:true});});
app.post('/api/tasks/:id/run',(q,s)=>{db.prepare("UPDATE tasks SET phase='implementation',started_at=COALESCE(started_at,unixepoch()),updated_at=unixepoch() WHERE id=?").run(q.params.id); db.prepare('INSERT INTO task_runs (id,task_id,status,started_at) VALUES (?,?,?,unixepoch())').run(randomUUID(),q.params.id,'running'); s.json({ok:true});});
app.post('/api/tasks/:id/stop',(q,s)=>{db.prepare("UPDATE task_runs SET status='stopped',ended_at=unixepoch() WHERE task_id=? AND status='running'").run(q.params.id); db.prepare("UPDATE tasks SET phase='validation',updated_at=unixepoch() WHERE id=?").run(q.params.id); s.json({ok:true});});
app.post('/api/tasks/:id/review',(q,s)=>{const {decision,comment=null}=q.body??{}; if(!['approved','hold','reject'].includes(decision)) return s.status(400).json({error:'decision invalid'}); const reviewer=db.prepare("SELECT id FROM users WHERE role IN ('owner','reviewer') LIMIT 1").get(); db.prepare('INSERT INTO reviews (id,task_id,reviewer_user_id,decision,comment) VALUES (?,?,?,?,?)').run(randomUUID(),q.params.id,reviewer.id,decision,comment); const phase = decision==='approved'?'done':decision==='hold'?'review':'revision'; db.prepare("UPDATE tasks SET phase=?, completed_at=CASE WHEN ?='done' THEN unixepoch() ELSE completed_at END, updated_at=unixepoch() WHERE id=?").run(phase,phase,q.params.id); s.json({ok:true,phase});});
app.get('/api/messages',(q,s)=>{const msgs=q.query.task_id?db.prepare('SELECT * FROM messages WHERE task_id=? ORDER BY created_at').all(String(q.query.task_id)):db.prepare('SELECT * FROM messages ORDER BY created_at DESC LIMIT 200').all(); s.json({messages:msgs});});
app.post('/api/messages',(q,s)=>{const {task_id=null,receiver_type='all',receiver_id=null,message_type='chat',content}=q.body??{}; if(!content) return s.status(400).json({error:'content is required'}); const id=randomUUID(); db.prepare('INSERT INTO messages (id,task_id,sender_type,sender_id,receiver_type,receiver_id,message_type,content) VALUES (?,?,?,?,?,?,?,?)').run(id,task_id,'user',null,receiver_type,receiver_id,message_type,content); s.status(201).json({id});});
app.get('/api/audit-logs',(_q,s)=>s.json({logs:db.prepare('SELECT * FROM audit_logs ORDER BY created_at DESC LIMIT 300').all()}));
app.get('*', (_q,s)=>s.sendFile(path.join(webDir,'index.html')));

const port = Number(process.env.PORT ?? 8790);
app.listen(port,'127.0.0.1',()=>console.log(`Game Orchestrator API running at http://127.0.0.1:${port}`));
