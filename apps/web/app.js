const $ = (id) => document.getElementById(id);

async function api(path, options) {
  const res = await fetch(path, {
    headers: { 'content-type': 'application/json' },
    ...options,
  });
  if (!res.ok) throw new Error(await res.text());
  if (res.status === 204) return null;
  return res.json();
}

async function refresh() {
  const [health, depRes, agentRes, taskRes, msgRes, logRes] = await Promise.all([
    api('/api/health'),
    api('/api/departments'),
    api('/api/agents'),
    api('/api/tasks'),
    api('/api/messages'),
    api('/api/audit-logs'),
  ]);

  $('health').textContent = health.ok ? 'healthy' : 'down';

  $('agentDept').innerHTML = depRes.departments
    .map((d) => `<option value="${d.id}">${d.icon || ''} ${d.name}</option>`)
    .join('');

  $('agents').innerHTML = agentRes.agents
    .map((a) => `<li>${a.name} <span class="pill">${a.department_id}</span> <span class="pill">${a.role_level}</span></li>`)
    .join('');

  $('tasks').innerHTML = taskRes.tasks
    .map((t) => `<li>${t.title} <span class="pill">${t.phase}</span></li>`)
    .join('');

  $('messages').innerHTML = msgRes.messages
    .slice(0, 20)
    .map((m) => `<li>${m.message_type}: ${m.content}</li>`)
    .join('');

  $('audit').innerHTML = logRes.logs
    .slice(0, 20)
    .map((l) => `<li>${l.action} (${l.target_type})</li>`)
    .join('');
}

$('createAgentBtn').addEventListener('click', async () => {
  const specialties = $('agentSpec').value
    .split(',')
    .map((s) => s.trim())
    .filter(Boolean);
  await api('/api/agents', {
    method: 'POST',
    body: JSON.stringify({
      name: $('agentName').value,
      role_level: $('agentRole').value,
      department_id: $('agentDept').value,
      provider: $('agentProvider').value,
      specialties,
    }),
  });
  $('agentName').value = '';
  $('agentSpec').value = '';
  await refresh();
});

$('createTaskBtn').addEventListener('click', async () => {
  await api('/api/tasks', {
    method: 'POST',
    body: JSON.stringify({
      title: $('taskTitle').value,
      acceptance_criteria: $('taskAc').value,
    }),
  });
  $('taskTitle').value = '';
  $('taskAc').value = '';
  await refresh();
});

$('sendMessageBtn').addEventListener('click', async () => {
  await api('/api/messages', {
    method: 'POST',
    body: JSON.stringify({
      receiver_type: 'all',
      message_type: 'directive',
      content: $('messageText').value,
    }),
  });
  $('messageText').value = '';
  await refresh();
});

refresh().catch((e) => {
  console.error(e);
  $('health').textContent = 'error';
});
