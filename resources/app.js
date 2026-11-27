(function () {
  let isRunning = false;
  let currentConfig = {};
  let currentAfkMinutes = 10;
  let currentScanSeconds = 15;
  let logsPaused = false;
  let isProgrammaticScroll = false;
  let pendingLogs = [];

  function sendCommand(cmd) {
    if (window.ipc && window.ipc.postMessage) {
      window.ipc.postMessage(JSON.stringify(cmd));
    }
  }

  function escapeHtml(str) {
    return String(str ?? '')
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;')
      .replace(/'/g, '&#39;');
  }

  function formatDuration(secs) {
    if (secs === null || secs === undefined) return '--';
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    const s = secs % 60;
    if (h > 0) {
      return `${String(h).padStart(2, '0')}h ${String(m).padStart(2, '0')}m`;
    }
    return `${String(m).padStart(2, '0')}m ${String(s).padStart(2, '0')}s`;
  }

  const toggleMasterBtn = document.getElementById('toggleMasterBtn');
  const pulseDot = document.getElementById('pulseDot');
  const pulseLabel = document.getElementById('pulseLabel');
  const logsConsole = document.getElementById('logsConsole');
  const resumeLogsBtn = document.getElementById('resumeLogsBtn');
  const settingsModal = document.getElementById('settingsModal');
  const settingsError = document.getElementById('settingsError');

  toggleMasterBtn.addEventListener('click', () => {
    sendCommand({ type: isRunning ? 'stopAutomator' : 'startAutomator' });
  });

  document.getElementById('testJumpBtn').addEventListener('click', () => {
    sendCommand({ type: 'triggerTestJump' });
  });

  const ribbonEventEnabled = document.getElementById('ribbonEventEnabled');
  const ribbonEventToggleLabel = document.getElementById('ribbonEventToggleLabel');
  const ribbonEventKeywords = document.getElementById('ribbonEventKeywords');

  function sendEventTrackingUpdate() {
    const keywords = ribbonEventKeywords.value
      .split(',')
      .map(s => s.trim())
      .filter(Boolean);
    const enabled = Boolean(ribbonEventEnabled.checked);
    ribbonEventToggleLabel.textContent = enabled ? 'ON' : 'OFF';
    ribbonEventToggleLabel.style.color = enabled ? '#34d399' : '#64748b';
    sendCommand({ type: 'setEventTracking', enabled, keywords });
  }

  ribbonEventEnabled.addEventListener('change', sendEventTrackingUpdate);
  ribbonEventKeywords.addEventListener('change', sendEventTrackingUpdate);
  ribbonEventKeywords.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      sendEventTrackingUpdate();
      ribbonEventKeywords.blur();
    }
  });

  document.getElementById('scanUpBtn').addEventListener('click', () => {
    const next = Math.min(60, currentScanSeconds + 1);
    if (next !== currentScanSeconds) {
      currentScanSeconds = next;
      document.getElementById('ribbonScanSeconds').textContent = `${next}s`;
      sendCommand({ type: 'setScanInterval', seconds: next });
    }
  });

  document.getElementById('scanDownBtn').addEventListener('click', () => {
    const next = Math.max(1, currentScanSeconds - 1);
    if (next !== currentScanSeconds) {
      currentScanSeconds = next;
      document.getElementById('ribbonScanSeconds').textContent = `${next}s`;
      sendCommand({ type: 'setScanInterval', seconds: next });
    }
  });

  document.getElementById('afkUpBtn').addEventListener('click', () => {
    const next = Math.min(19, currentAfkMinutes + 1);
    if (next !== currentAfkMinutes) {
      currentAfkMinutes = next;
      document.getElementById('ribbonAfkMinutes').textContent = `${next}m`;
      sendCommand({ type: 'setAntiAfkInterval', minutes: next });
    }
  });

  document.getElementById('afkDownBtn').addEventListener('click', () => {
    const next = Math.max(1, currentAfkMinutes - 1);
    if (next !== currentAfkMinutes) {
      currentAfkMinutes = next;
      document.getElementById('ribbonAfkMinutes').textContent = `${next}m`;
      sendCommand({ type: 'setAntiAfkInterval', minutes: next });
    }
  });

  // Macro controls wired to backend recorder & replayer
  const macroStatBox = document.getElementById('macroStatBox');
  const ribbonMacroEnabled = document.getElementById('ribbonMacroEnabled');
  const ribbonMacroToggleLabel = document.getElementById('ribbonMacroToggleLabel');
  const macroRecordBtn = document.getElementById('macroRecordBtn');
  const macroWindowSelect = document.getElementById('macroWindowSelect');
  const macroPlayBtn = document.getElementById('macroPlayBtn');
  const macroClearBtn = document.getElementById('macroClearBtn');
  const macroSummaryLabel = document.getElementById('macroSummaryLabel');
  let macroRecordingState = false;
  let macroPlayingState = false;

  ribbonMacroEnabled.addEventListener('change', () => {
    const on = ribbonMacroEnabled.checked;
    ribbonMacroToggleLabel.textContent = on ? 'ON' : 'OFF';
    ribbonMacroToggleLabel.style.color = on ? '#fbbf24' : '#94a3b8';
    sendCommand({ type: 'setMacroEnabled', enabled: on });
  });

  macroRecordBtn.addEventListener('click', () => {
    if (macroRecordingState) {
      sendCommand({ type: 'stopMacroRecording' });
    } else {
      const idx = parseInt(macroWindowSelect.value, 10);
      sendCommand({ type: 'startMacroRecording', windowIndex: Number.isNaN(idx) ? 0 : idx });
    }
  });

  macroPlayBtn.addEventListener('click', () => {
    const idx = parseInt(macroWindowSelect.value, 10);
    sendCommand({ type: 'testMacro', windowIndex: Number.isNaN(idx) ? 0 : idx });
  });

  macroClearBtn.addEventListener('click', () => {
    if (macroPlayingState) {
      sendCommand({ type: 'stopMacroPlayback' });
    } else {
      sendCommand({ type: 'clearMacro' });
    }
  });

  function renderLogs(logs) {
    if (logs && logs.length > 0) {
      logsConsole.innerHTML = logs.map(entry => {
        const cls = entry.level === 'error'
          ? 'log-error'
          : entry.level === 'warn'
            ? 'log-warn'
            : entry.level === 'action'
              ? 'log-action'
              : 'log-info';
        return `<div class="${cls} mono">[${escapeHtml(entry.timestamp)}] ${escapeHtml(entry.message)}</div>`;
      }).join('');
    } else {
      logsConsole.innerHTML = '<div class="mono" style="color:#64748b">// Console cleared</div>';
    }
    isProgrammaticScroll = true;
    logsConsole.scrollTop = logsConsole.scrollHeight;
    requestAnimationFrame(() => {
      isProgrammaticScroll = false;
    });
  }

  logsConsole.addEventListener('scroll', () => {
    if (isProgrammaticScroll) return;
    const distanceFromBottom = logsConsole.scrollHeight - logsConsole.scrollTop - logsConsole.clientHeight;
    if (distanceFromBottom > 12 && !logsPaused) {
      logsPaused = true;
      resumeLogsBtn.classList.remove('hidden');
    }
  });

  resumeLogsBtn.addEventListener('click', () => {
    logsPaused = false;
    resumeLogsBtn.classList.add('hidden');
    renderLogs(pendingLogs);
  });

  document.getElementById('downloadLogsBtn').addEventListener('click', () => {
    sendCommand({ type: 'exportLogsJsonl' });
  });

  document.getElementById('clearLogsBtn').addEventListener('click', () => {
    logsPaused = false;
    pendingLogs = [];
    resumeLogsBtn.classList.add('hidden');
    logsConsole.innerHTML = '';
    sendCommand({ type: 'clearLogs' });
  });

  document.getElementById('openSettingsBtn').addEventListener('click', () => {
    settingsError.classList.add('hidden');
    sendCommand({ type: 'getConfig' });
    settingsModal.classList.remove('hidden');
  });

  document.getElementById('closeSettingsBtn').addEventListener('click', () => {
    settingsModal.classList.add('hidden');
  });
  document.getElementById('cancelSettingsBtn').addEventListener('click', () => {
    settingsModal.classList.add('hidden');
  });

  document.getElementById('settingsForm').addEventListener('submit', (e) => {
    e.preventDefault();
    settingsError.classList.add('hidden');

    const splitList = (id) => document.getElementById(id).value.split(',').map(s => s.trim()).filter(Boolean);
    const prevUiStrings = currentConfig.uiStrings || {};
    const appTitle = prevUiStrings.appTitle || 'Roblox Automator';

    const updated = {
      ...currentConfig,
      checkIntervalSeconds: parseFloat(document.getElementById('cfgInterval').value) || 15.0,
      uiRefreshIntervalMs: Math.round((parseFloat(document.getElementById('cfgUiRefreshSec').value) || 2.0) * 1000),
      reconnectCooldownSeconds: parseFloat(document.getElementById('cfgCooldown').value) || 30.0,
      targetProcessNames: splitList('cfgTargetProcesses'),
      reconnectButtonPhrases: splitList('cfgReconnectButtonPhrases'),
      disconnectKeywords: splitList('cfgDisconnectKeywords'),
      ignorePhrases: splitList('cfgIgnorePhrases'),
      antiAfkEnabled: document.getElementById('cfgAntiAfkEnabled').checked,
      antiAfkIntervalSeconds: (parseInt(document.getElementById('cfgAntiAfkMinutes').value, 10) || 10) * 60.0,
      uiStrings: {
        ...prevUiStrings,
        appTitle
      }
    };

    sendCommand({ type: 'saveConfig', config: updated });
  });

  function populateConfig(cfg) {
    currentConfig = cfg;
    document.getElementById('cfgInterval').value = cfg.checkIntervalSeconds ?? 15.0;
    document.getElementById('cfgUiRefreshSec').value = ((cfg.uiRefreshIntervalMs ?? 2000) / 1000);
    document.getElementById('cfgCooldown').value = cfg.reconnectCooldownSeconds ?? 30.0;
    document.getElementById('cfgTargetProcesses').value = (cfg.targetProcessNames ?? []).join(', ');
    const buttonPhrases = cfg.reconnectButtonPhrases || [];
    document.getElementById('cfgReconnectButtonPhrases').value = Array.isArray(buttonPhrases) ? buttonPhrases.join(', ') : buttonPhrases;
    document.getElementById('cfgDisconnectKeywords').value = (cfg.disconnectKeywords ?? []).join(', ');
    document.getElementById('cfgIgnorePhrases').value = (cfg.ignorePhrases ?? []).join(', ');
    document.getElementById('cfgAntiAfkEnabled').checked = cfg.antiAfkEnabled ?? true;
    document.getElementById('cfgAntiAfkMinutes').value = Math.round((cfg.antiAfkIntervalSeconds ?? 600) / 60);

    const appTitle = cfg.uiStrings?.appTitle || 'Roblox Automator';
    document.title = appTitle;
  }

  function renderTelemetry(data) {
    isRunning = Boolean(data.running);

    if (isRunning) {
      toggleMasterBtn.className = 'btn btn-danger mono';
      toggleMasterBtn.textContent = 'Stop All';
      pulseDot.classList.add('active');
      if (data.sessionUptimeSecs !== null && data.sessionUptimeSecs !== undefined) {
        pulseLabel.textContent = formatDuration(data.sessionUptimeSecs);
        pulseLabel.style.color = '#34d399';
      } else {
        pulseLabel.textContent = 'RUNNING';
        pulseLabel.style.color = '#34d399';
      }
    } else {
      toggleMasterBtn.className = 'btn btn-primary mono';
      toggleMasterBtn.textContent = 'Start All';
      pulseDot.classList.remove('active');
      pulseLabel.textContent = 'STOPPED';
      pulseLabel.style.color = '#94a3b8';
    }

    if (data.appVersion) {
      const verEl = document.getElementById('appVersionBadge');
      if (verEl) {
        verEl.textContent = `v${data.appVersion}`;
      }
    }

    const winCount = data.instances ? data.instances.length : 0;
    document.getElementById('activeWinsCount').textContent = winCount;
    const winsUnitEl = document.getElementById('activeWinsUnit');
    if (winsUnitEl) {
      winsUnitEl.textContent = winCount === 1 ? 'window' : 'windows';
    }

    const restarts = data.totalRestarts ?? 0;
    document.getElementById('ribbonTotalRestarts').textContent = `${restarts} ${restarts === 1 ? 'rejoin' : 'rejoins'}`;

    if (macroWindowSelect) {
      const prevVal = macroWindowSelect.value;
      const targetCount = Math.max(winCount, 1);
      if (macroWindowSelect.options.length !== targetCount) {
        macroWindowSelect.innerHTML = Array.from({ length: targetCount }, (_, i) => {
          return `<option value="${i}">#${i + 1}</option>`;
        }).join('');
        if (prevVal && Array.from(macroWindowSelect.options).some(o => o.value === prevVal)) {
          macroWindowSelect.value = prevVal;
        }
      }
    }

    if (ribbonMacroEnabled) {
      const mOn = Boolean(data.macroEnabled);
      ribbonMacroEnabled.checked = mOn;
      ribbonMacroToggleLabel.textContent = mOn ? 'ON' : 'OFF';
      ribbonMacroToggleLabel.style.color = mOn ? '#fbbf24' : '#94a3b8';
    }

    macroRecordingState = Boolean(data.macroRecording);
    macroPlayingState = Boolean(data.macroPlaying);
    macroRecordBtn.classList.toggle('recording', macroRecordingState);
    macroStatBox.classList.toggle('recording', macroRecordingState);
    macroRecordBtn.innerHTML = macroRecordingState ? '&#9632; Stop' : '&#9679; Rec';
    macroRecordBtn.disabled = macroPlayingState;

    const stepCount = data.macroStepCount ?? 0;
    const durSecs = ((data.macroDurationMs ?? 0) / 1000).toFixed(1);
    if (macroRecordingState && data.macroCountdownSecs) {
      macroSummaryLabel.textContent = `Starting in ${data.macroCountdownSecs}...`;
      macroSummaryLabel.style.color = '#fbbf24';
    } else if (macroRecordingState) {
      macroSummaryLabel.textContent = 'Recording (leave win)...';
      macroSummaryLabel.style.color = '#fb7185';
    } else if (macroPlayingState) {
      macroSummaryLabel.textContent = 'Playing macro...';
      macroSummaryLabel.style.color = '#34d399';
    } else if (stepCount > 0) {
      macroSummaryLabel.textContent = `${stepCount} steps · ${durSecs}s`;
      macroSummaryLabel.style.color = '#34d399';
    } else {
      macroSummaryLabel.textContent = 'No macro';
      macroSummaryLabel.style.color = '';
    }
    macroPlayBtn.disabled = macroRecordingState || macroPlayingState || stepCount === 0;
    if (macroPlayingState) {
      macroClearBtn.innerHTML = '&#9632;';
      macroClearBtn.classList.add('macro-stop-btn');
      macroClearBtn.title = 'Stop macro playback';
      macroClearBtn.disabled = false;
    } else {
      macroClearBtn.innerHTML = '&#10005;';
      macroClearBtn.classList.remove('macro-stop-btn');
      macroClearBtn.title = 'Clear recorded macro';
      macroClearBtn.disabled = macroRecordingState || stepCount === 0;
    }

    if (data.scanIntervalSeconds) {
      currentScanSeconds = data.scanIntervalSeconds;
      document.getElementById('ribbonScanSeconds').textContent = `${currentScanSeconds}s`;
    }
    const scanStatusEl = document.getElementById('ribbonScanStatus');
    if (!isRunning) {
      scanStatusEl.textContent = 'PAUSED';
      scanStatusEl.style.color = '#94a3b8';
    } else {
      const rem = data.nextScanRemainingSecs ?? 0;
      scanStatusEl.textContent = `${rem}s`;
      scanStatusEl.style.color = '#38bdf8';
    }

    const eventOn = Boolean(data.autoTeleportToEvent);
    ribbonEventEnabled.checked = eventOn;
    ribbonEventToggleLabel.textContent = eventOn ? 'ON' : 'OFF';
    ribbonEventToggleLabel.style.color = eventOn ? '#34d399' : '#64748b';
    if (document.activeElement !== ribbonEventKeywords && Array.isArray(data.eventTargetKeywords)) {
      ribbonEventKeywords.value = data.eventTargetKeywords.join(', ');
    }

    if (data.antiAfkIntervalMinutes) {
      currentAfkMinutes = data.antiAfkIntervalMinutes;
      document.getElementById('ribbonAfkMinutes').textContent = `${currentAfkMinutes}m`;
    }

    const afkStatusEl = document.getElementById('ribbonAfkStatus');
    if (!data.antiAfkEnabled) {
      afkStatusEl.textContent = 'OFF';
      afkStatusEl.style.color = '#64748b';
    } else if (!isRunning) {
      afkStatusEl.textContent = 'PAUSED';
      afkStatusEl.style.color = '#94a3b8';
    } else {
      afkStatusEl.textContent = formatDuration(data.nextAfkRemainingSecs);
      afkStatusEl.style.color = '#38bdf8';
    }

    const grid = document.getElementById('cctvGrid');
    const count = data.instances ? data.instances.length : 0;
    const cols = count <= 1 ? 1 : (count <= 4 ? count : (count <= 6 || count === 9 ? 3 : 4));
    const rows = count > 0 ? Math.ceil(count / cols) : 1;
    grid.style.setProperty('--grid-cols', String(cols));
    grid.dataset.count = String(count);
    grid.dataset.rows = String(rows);

    if (count > 0) {
      grid.innerHTML = data.instances.map((inst, idx) => {
        const stateStr = String(inst.state || 'IDLE');
        let badgeClass = 'badge-healthy';
        let cardClass = '';
        if (stateStr === 'IDLE' || stateStr === 'DISCOVERING') {
          badgeClass = 'badge-idle';
        } else if (stateStr === 'DISCONNECTED' || stateStr === 'ERROR') {
          badgeClass = 'badge-error';
          cardClass = stateStr === 'ERROR' ? 'state-error' : 'state-disconnected';
        } else if (stateStr === 'RECONNECTING' || stateStr === 'NAVIGATING') {
          badgeClass = 'badge-warn';
          cardClass = 'state-reconnecting';
        }

        const thumbHtml = inst.screenshotBase64
          ? `<img src="data:image/jpeg;base64,${escapeHtml(inst.screenshotBase64)}" alt="Roblox #${idx + 1}">`
          : `<div class="mono" style="color:#64748b;font-size:13px;">No Frame</div>`;

        const eventCropHtml = inst.eventCropBase64
          ? `<img src="data:image/jpeg;base64,${escapeHtml(inst.eventCropBase64)}" alt="Target crop" class="event-crop-overlay">`
          : '';

        const reconnectCropHtml = inst.reconnectCropBase64
          ? `<img src="data:image/jpeg;base64,${escapeHtml(inst.reconnectCropBase64)}" alt="Reconnect crop" class="reconnect-crop-overlay">`
          : '';

        const errHtml = inst.failureReason
          ? `<div class="card-error-line mono" title="${escapeHtml(inst.failureReason)}">${escapeHtml(inst.failureReason)}</div>`
          : '';

        const cooldownTag = inst.cooldownRemainingSecs
          ? `Cooldown: ${inst.cooldownRemainingSecs}s`
          : '';

        const thumbClass = inst.failureReason ? 'card-thumb has-error' : 'card-thumb';

        return `
          <div class="card ${cardClass}">
            <div class="${thumbClass}">
              ${thumbHtml}
              <div class="card-overlay-top">
                <span class="tag mono">#${idx + 1}</span>
                <span class="badge mono ${badgeClass}">${escapeHtml(stateStr)}</span>
              </div>
              <div class="card-overlay-bottom">
                <span class="tag mono">${escapeHtml(inst.eventStatus || 'Standby')}</span>
              </div>
              ${eventCropHtml}
              ${reconnectCropHtml}
              ${errHtml}
            </div>
            <div class="card-footer mono">
              <span>${escapeHtml(cooldownTag)}</span>
              <span>Restarts: <strong style="color:#f1f5f9">${escapeHtml(inst.restarts ?? 0)}</strong></span>
            </div>
          </div>
        `;
      }).join('');
    } else {
      grid.innerHTML = `
      <div class="empty-state mono">
          <span>No active game windows detected</span>
        </div>
      `;
    }

    pendingLogs = data.logs || [];
    if (!logsPaused) {
      renderLogs(pendingLogs);
    }
  }

  // Single typed event entrypoint invoked by Rust EventLoopProxy
  window.handleNativeEvent = function (event) {
    if (!event || !event.type) return;
    switch (event.type) {
      case 'telemetrySnapshot':
        renderTelemetry(event.payload);
        break;
      case 'configLoaded':
        populateConfig(event.payload);
        break;
      case 'configSaved':
        populateConfig(event.payload);
        settingsModal.classList.add('hidden');
        break;
      case 'logsJsonlExported': {
        const filename = event.payload?.filename || 'roblox-automator-logs.jsonl';
        const content = event.payload?.content || '';
        const a = document.createElement('a');
        a.href = 'data:application/x-ndjson;charset=utf-8,' + encodeURIComponent(content);
        a.download = filename;
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        break;
      }
      case 'actionError':
        settingsError.textContent = event.payload?.message || 'Validation error';
        settingsError.classList.remove('hidden');
        break;
    }
  };
})();
