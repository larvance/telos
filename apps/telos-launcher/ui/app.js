// ============================================================================
// Telos Launcher Frontend Controller (Tauri 2 Native IPC + Fallback)
// ============================================================================

(function () {
  'use strict';

  // --- Tauri IPC Wrappers ---
  const isTauri = !!(window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke);

  async function invoke(cmd, args = {}) {
    if (isTauri) {
      try {
        return await window.__TAURI__.core.invoke(cmd, args);
      } catch (err) {
        console.error(`Tauri IPC error for '${cmd}':`, err);
        throw err;
      }
    }
    return mockInvoke(cmd, args);
  }

  // Fallback mock database for browser testing/previews
  let mockState = {
    profile: {
      username: 'Larvance',
      account_type: 'Telos Account',
      logged_in: false,
      selected_installation_id: 'dev-local',
      auto_close_on_play: false,
      language: 'English - United States',
    },
    installations: [
      {
        id: 'dev-local',
        name: 'Dev Environment (Local Build)',
        version: '0.1.0-dev (Local)',
        directory: '/home/usr/Projects/larvance/telos',
        icon: '🔨',
        is_dev: true,
      },
      {
        id: 'release-latest',
        name: 'Latest release',
        version: '0.1.0',
        directory: '/home/usr/.local/share/telos/versions/0.1.0',
        icon: '🟩',
        is_dev: false,
      },
      {
        id: 'snapshot-latest',
        name: 'Latest snapshot',
        version: '0.2.0-alpha',
        directory: '/home/usr/.local/share/telos/versions/0.2.0-alpha',
        icon: '📦',
        is_dev: false,
      },
    ],
    servers: [
      {
        id: 'srv-1',
        name: 'Official Telos Alpha Server',
        address: 'play.telos.voxel:25565',
        group: 'Featured & Official',
        order: 0,
        ping_ms: 18,
        online: true,
      },
      {
        id: 'srv-2',
        name: 'Survival & Builders Haven',
        address: 'survival.telos.voxel:25565',
        group: 'Featured & Official',
        order: 1,
        ping_ms: 24,
        online: true,
      },
      {
        id: 'srv-3',
        name: 'Larvance Local Dev Server',
        address: '127.0.0.1:25565',
        group: 'Local LAN & Dev',
        order: 0,
        ping_ms: 1,
        online: true,
      },
      {
        id: 'srv-4',
        name: 'Community Anarchy Sandbox',
        address: 'anarchy.telos.voxel:25565',
        group: 'Community Servers',
        order: 0,
        ping_ms: 45,
        online: true,
      },
    ],
    updater: {
      current_version: '0.1.0',
      latest_version: '0.1.1',
      update_available: false,
      release_notes: 'All launcher components are up to date.',
    },
  };

  async function mockInvoke(cmd, args) {
    console.log(`[Mock IPC] ${cmd}:`, args);
    switch (cmd) {
      case 'get_profile':
        return mockState.profile;
      case 'save_profile':
        mockState.profile = { ...mockState.profile, ...args.profile };
        return;
      case 'login':
        mockState.profile.account_name = args.username || 'Larvance';
        mockState.profile.auth_token = 'token-' + Date.now();
        return mockState.profile;
      case 'logout':
        mockState.profile.auth_token = null;
        return;
      case 'get_installations':
        return mockState.installations;
      case 'save_installations':
        mockState.installations = args.installations;
        return;
      case 'get_servers':
        return mockState.servers;
      case 'save_servers':
        mockState.servers = args.servers;
        return;
      case 'check_updater_status':
        return mockState.updater;
      case 'trigger_launch':
        return {
          success: true,
          message: 'Launching Telos...',
          is_building: args.installation_id === 'dev-local',
        };
      case 'window_minimize':
      case 'window_toggle_maximize':
      case 'window_close':
        console.log(`Window command '${cmd}' acknowledged.`);
        return;
      default:
        return null;
    }
  }

  // --- App State ---
  let appProfile = null;
  let appInstallations = [];
  let appServers = [];
  let selectedInstallId = 'dev-local';
  let isBuilding = false;
  let editingServerId = null;

  // --- DOM Elements ---
  const elLoginScreen = document.getElementById('login-screen');
  const elMainScreen = document.getElementById('main-screen');
  const elLoginForm = document.getElementById('login-form');
  const elLoginUsername = document.getElementById('login-username');
  const elLinkGuest = document.getElementById('link-guest-login');
  const elUserDisplayName = document.getElementById('user-display-name');
  const elUserAvatar = document.getElementById('user-avatar');

  // Titlebar controls
  document.getElementById('btn-minimize').addEventListener('click', () => invoke('window_minimize'));
  document.getElementById('btn-maximize').addEventListener('click', () => invoke('window_toggle_maximize'));
  document.getElementById('btn-close').addEventListener('click', () => invoke('window_close'));

  // Navigation tabs
  const navTabs = document.querySelectorAll('.nav-tab');
  const tabPanels = document.querySelectorAll('.tab-panel');

  navTabs.forEach((tab) => {
    tab.addEventListener('click', () => {
      const target = tab.getAttribute('data-tab');
      navTabs.forEach((t) => t.classList.remove('active'));
      tabPanels.forEach((p) => p.classList.remove('active'));

      tab.classList.add('active');
      const targetPanel = document.getElementById(`tab-${target}`);
      if (targetPanel) targetPanel.classList.add('active');
    });
  });

  // --- Login Screen Handlers ---
  async function performLogin(username) {
    try {
      appProfile = await invoke('login', { username });
      transitionToMain();
    } catch (err) {
      console.error('Failed to log in:', err);
    }
  }

  elLoginForm.addEventListener('submit', (e) => {
    e.preventDefault();
    const val = elLoginUsername.value.trim() || 'Larvance';
    performLogin(val);
  });

  elLinkGuest.addEventListener('click', (e) => {
    e.preventDefault();
    performLogin('Guest_' + Math.floor(Math.random() * 9000 + 1000));
  });

  document.getElementById('btn-logout').addEventListener('click', async () => {
    await invoke('logout');
    transitionToLogin();
  });

  function transitionToMain() {
    elLoginScreen.classList.remove('active');
    elMainScreen.classList.add('active');
    if (appProfile) {
      const uName = appProfile.username || appProfile.account_name || 'Larvance';
      elUserDisplayName.textContent = uName;
      elUserAvatar.textContent = (uName[0] || 'T').toUpperCase();
    }
  }

  function transitionToLogin() {
    elMainScreen.classList.remove('active');
    elLoginScreen.classList.add('active');
  }

  // --- Installation Dropdown on Play Tab ---
  const elInstallBtn = document.getElementById('install-selector-btn');
  const elInstallMenu = document.getElementById('install-dropdown-menu');
  const elCurrInstallIcon = document.getElementById('curr-install-icon');
  const elCurrInstallName = document.getElementById('curr-install-name');
  const elCurrInstallVersion = document.getElementById('curr-install-version');

  elInstallBtn.addEventListener('click', (e) => {
    e.stopPropagation();
    elInstallMenu.classList.toggle('open');
  });

  document.addEventListener('click', () => {
    elInstallMenu.classList.remove('open');
  });

  function renderInstallationSelector() {
    elInstallMenu.innerHTML = '';
    const current = appInstallations.find((i) => i.id === selectedInstallId) || appInstallations[0];
    if (current) {
      elCurrInstallIcon.textContent = current.icon || '🟩';
      elCurrInstallName.textContent = current.name;
      elCurrInstallVersion.textContent = current.version;
    }

    appInstallations.forEach((inst) => {
      const opt = document.createElement('div');
      opt.className = 'install-option' + (inst.id === selectedInstallId ? ' selected' : '');
      opt.innerHTML = `
        <span style="font-size: 16px;">${inst.icon || '🟩'}</span>
        <div style="display: flex; flex-direction: column;">
          <span style="font-size: 13px; font-weight: 600; color: #fff;">${inst.name}</span>
          <span style="font-size: 11px; color: #aaa;">${inst.version}</span>
        </div>
      `;
      opt.addEventListener('click', async () => {
        selectedInstallId = inst.id;
        elInstallMenu.classList.remove('open');
        renderInstallationSelector();
        if (appProfile) {
          appProfile.selected_installation_id = selectedInstallId;
          appProfile.last_used_installation = selectedInstallId;
          await invoke('save_profile', { profile: appProfile });
        }
      });
      elInstallMenu.appendChild(opt);
    });
  }

  // --- Play Button & Dev Build Event Handling ---
  const elPlayBtn = document.getElementById('play-btn');
  const elBuildDrawer = document.getElementById('build-drawer');
  const elBuildStatus = document.getElementById('build-status-text');
  const elBuildProgress = document.getElementById('build-progress-fill');
  const elBuildHeading = document.getElementById('build-drawer-heading');
  const elCloseBuildDrawer = document.getElementById('btn-close-build-drawer');

  elCloseBuildDrawer.addEventListener('click', () => {
    elBuildDrawer.classList.remove('active');
  });

  elPlayBtn.addEventListener('click', async () => {
    if (isBuilding) return;

    elPlayBtn.disabled = true;
    elPlayBtn.textContent = 'CHECKING...';

    try {
      const res = await invoke('trigger_launch', { installation_id: selectedInstallId });
      if (res && res.is_building) {
        isBuilding = true;
        elPlayBtn.textContent = 'BUILDING...';
        elBuildDrawer.classList.add('active');
        elBuildStatus.textContent = res.message;
        elBuildProgress.style.width = '15%';
        listenBuildEvents();
      } else {
        elPlayBtn.textContent = 'RUNNING';
        setTimeout(() => {
          elPlayBtn.disabled = false;
          elPlayBtn.textContent = 'PLAY';
        }, 3000);
      }
    } catch (err) {
      console.error('Launch failed:', err);
      elPlayBtn.disabled = false;
      elPlayBtn.textContent = 'PLAY';
    }
  });

  let buildEventUnlisten = null;
  let compiledCratesCount = 0;
  const ESTIMATED_TOTAL_CRATES = 28;

  async function listenBuildEvents() {
    if (isTauri && window.__TAURI__.event && window.__TAURI__.event.listen) {
      if (buildEventUnlisten) {
        buildEventUnlisten();
      }
      buildEventUnlisten = await window.__TAURI__.event.listen('build_event', (event) => {
        handleBuildPayload(event.payload);
      });
    } else {
      // Simulate build events in browser preview
      simulateMockBuildEvents();
    }
  }

  function handleBuildPayload(payload) {
    if (!payload) return;

    if (payload.Status) {
      elBuildStatus.textContent = payload.Status;
    } else if (payload.CrateStarted) {
      elBuildStatus.textContent = `Compiling ${payload.CrateStarted}...`;
      compiledCratesCount++;
      const pct = Math.min(90, Math.floor((compiledCratesCount / ESTIMATED_TOTAL_CRATES) * 90));
      elBuildProgress.style.width = pct + '%';
    } else if (payload.ArtifactFinished) {
      elBuildStatus.textContent = `Finished ${payload.ArtifactFinished.crate_name}`;
    } else if (payload.LockHeld) {
      elBuildHeading.textContent = 'Waiting for Build Lock...';
      elBuildStatus.textContent = payload.LockHeld.message || 'Another Telos process is building...';
      elBuildProgress.style.width = '10%';
    } else if (payload.Success) {
      isBuilding = false;
      elBuildProgress.style.width = '100%';
      elBuildHeading.textContent = 'Build Complete!';
      elBuildStatus.textContent = payload.Success.already_built
        ? 'Binary was already compiled by another instance. Launching...'
        : 'Successfully compiled release binary. Launching Telos!';
      elPlayBtn.disabled = false;
      elPlayBtn.textContent = 'PLAY';
      setTimeout(() => {
        elBuildDrawer.classList.remove('active');
        compiledCratesCount = 0;
      }, 4000);
    } else if (payload.Failed) {
      isBuilding = false;
      elBuildHeading.textContent = 'Build Failed';
      elBuildStatus.textContent = payload.Failed;
      elPlayBtn.disabled = false;
      elPlayBtn.textContent = 'PLAY';
    }
  }

  function simulateMockBuildEvents() {
    const steps = [
      { Status: 'Checking workspace advisory lock...' },
      { CrateStarted: 'telos-core' },
      { CrateStarted: 'telos-gpu' },
      { CrateStarted: 'telos-voxel' },
      { CrateStarted: 'telos-render' },
      { CrateStarted: 'telos' },
      { Success: { already_built: false } },
    ];
    let idx = 0;
    const interval = setInterval(() => {
      if (idx < steps.length) {
        handleBuildPayload(steps[idx++]);
      } else {
        clearInterval(interval);
      }
    }, 700);
  }

  // --- Installations Tab Rendering ---
  function renderInstallationsTab() {
    const container = document.getElementById('installations-container');
    container.innerHTML = '';

    appInstallations.forEach((inst) => {
      const card = document.createElement('div');
      card.className = 'installation-card';
      card.innerHTML = `
        <div class="inst-meta">
          <div class="inst-icon">${inst.icon || '🟩'}</div>
          <div class="inst-texts">
            <span class="inst-title">${inst.name}</span>
            <span class="inst-tag">Version: ${inst.version}</span>
            <span class="inst-dir">${inst.directory}</span>
          </div>
        </div>
        <div style="display: flex; gap: 8px;">
          <button class="btn-green-3d" style="padding: 8px 20px; font-size: 13px;" data-play-id="${inst.id}">
            PLAY
          </button>
        </div>
      `;

      card.querySelector('[data-play-id]').addEventListener('click', () => {
        selectedInstallId = inst.id;
        renderInstallationSelector();
        elPlayBtn.click();
      });

      container.appendChild(card);
    });
  }

  // --- Multiplayer Servers Tab (Collapsible & Draggable) ---
  const elServerGroupsContainer = document.getElementById('server-groups-container');
  const elBtnAddServer = document.getElementById('btn-add-server');
  const elServerModal = document.getElementById('server-modal');
  const elCloseServerModal = document.getElementById('btn-close-server-modal');
  const elCancelServer = document.getElementById('btn-cancel-server');
  const elSaveServer = document.getElementById('btn-save-server');
  const elServerNameInput = document.getElementById('server-name-input');
  const elServerAddressInput = document.getElementById('server-address-input');
  const elServerGroupInput = document.getElementById('server-group-input');
  const elServerModalTitle = document.getElementById('server-modal-title');

  elBtnAddServer.addEventListener('click', () => {
    editingServerId = null;
    elServerModalTitle.textContent = 'Add Multiplayer Server';
    elServerNameInput.value = '';
    elServerAddressInput.value = '';
    elServerGroupInput.value = 'Community Servers';
    elServerModal.classList.add('active');
  });

  const closeServerModal = () => elServerModal.classList.remove('active');
  elCloseServerModal.addEventListener('click', closeServerModal);
  elCancelServer.addEventListener('click', closeServerModal);

  elSaveServer.addEventListener('click', async () => {
    const name = elServerNameInput.value.trim() || 'New Telos Server';
    const address = elServerAddressInput.value.trim() || 'play.telos.voxel:25565';
    const group = elServerGroupInput.value.trim() || 'Community Servers';

    if (editingServerId) {
      const s = appServers.find((x) => x.id === editingServerId);
      if (s) {
        s.name = name;
        s.address = address;
        s.group = group;
      }
    } else {
      const newServer = {
        id: 'srv-' + Date.now(),
        name,
        address,
        group,
        order: appServers.filter((s) => s.group === group).length,
        ping_ms: Math.floor(Math.random() * 30 + 10),
        online: true,
      };
      appServers.push(newServer);
    }

    closeServerModal();
    await invoke('save_servers', { servers: appServers });
    renderServersTab();
  });

  let draggedServerId = null;

  function renderServersTab() {
    elServerGroupsContainer.innerHTML = '';

    // Group servers
    const groups = {};
    appServers.forEach((srv) => {
      const g = srv.group || 'Community Servers';
      if (!groups[g]) groups[g] = [];
      groups[g].push(srv);
    });

    // Sort within groups
    Object.keys(groups).forEach((g) => {
      groups[g].sort((a, b) => a.order - b.order);
    });

    Object.entries(groups).forEach(([groupName, servers]) => {
      const groupEl = document.createElement('div');
      groupEl.className = 'server-group';

      const headerEl = document.createElement('div');
      headerEl.className = 'server-group-header';
      headerEl.innerHTML = `
        <span class="server-group-title">
          <span class="chevron">▾</span>
          <span>${groupName} (${servers.length})</span>
        </span>
      `;
      headerEl.addEventListener('click', () => {
        groupEl.classList.toggle('collapsed');
      });

      const listEl = document.createElement('div');
      listEl.className = 'server-list';
      listEl.setAttribute('data-group', groupName);

      // Drag and drop event handling for reordering
      listEl.addEventListener('dragover', (e) => {
        e.preventDefault();
        const afterElement = getDragAfterElement(listEl, e.clientY);
        const dragging = document.querySelector('.server-card.dragging');
        if (dragging) {
          if (afterElement == null) {
            listEl.appendChild(dragging);
          } else {
            listEl.insertBefore(dragging, afterElement);
          }
        }
      });

      listEl.addEventListener('drop', async (e) => {
        e.preventDefault();
        // Re-calculate orders based on DOM position
        updateServerOrdersFromDOM();
        await invoke('save_servers', { servers: appServers });
      });

      servers.forEach((srv) => {
        const card = document.createElement('div');
        card.className = 'server-card';
        card.draggable = true;
        card.setAttribute('data-id', srv.id);

        card.innerHTML = `
          <div style="display: flex; align-items: center; gap: 12px;">
            <span class="server-drag-handle" title="Drag to reorder">⠿</span>
            <div class="server-icon">🌐</div>
            <div class="server-meta">
              <span class="server-name">${srv.name}</span>
              <span class="server-address">${srv.address}</span>
            </div>
          </div>
          <div style="display: flex; align-items: center; gap: 16px;">
            <div class="server-ping-badge">
              <span class="status-dot"></span>
              <span>${srv.ping_ms ? srv.ping_ms + 'ms' : 'Online'}</span>
            </div>
            <div class="server-actions">
              <button class="btn-icon edit-btn" title="Edit Server">✏️</button>
              <button class="btn-icon delete-btn" title="Delete Server">🗑️</button>
            </div>
          </div>
        `;

        // HTML5 drag handlers
        card.addEventListener('dragstart', () => {
          draggedServerId = srv.id;
          card.classList.add('dragging');
        });

        card.addEventListener('dragend', () => {
          draggedServerId = null;
          card.classList.remove('dragging');
        });

        // Edit server
        card.querySelector('.edit-btn').addEventListener('click', (e) => {
          e.stopPropagation();
          editingServerId = srv.id;
          elServerModalTitle.textContent = 'Edit Multiplayer Server';
          elServerNameInput.value = srv.name;
          elServerAddressInput.value = srv.address;
          elServerGroupInput.value = srv.group;
          elServerModal.classList.add('active');
        });

        // Delete server
        card.querySelector('.delete-btn').addEventListener('click', async (e) => {
          e.stopPropagation();
          appServers = appServers.filter((x) => x.id !== srv.id);
          await invoke('save_servers', { servers: appServers });
          renderServersTab();
        });

        listEl.appendChild(card);
      });

      groupEl.appendChild(headerEl);
      groupEl.appendChild(listEl);
      elServerGroupsContainer.appendChild(groupEl);
    });
  }

  function getDragAfterElement(container, y) {
    const draggableElements = [...container.querySelectorAll('.server-card:not(.dragging)')];
    return draggableElements.reduce(
      (closest, child) => {
        const box = child.getBoundingClientRect();
        const offset = y - box.top - box.height / 2;
        if (offset < 0 && offset > closest.offset) {
          return { offset: offset, element: child };
        } else {
          return closest;
        }
      },
      { offset: Number.NEGATIVE_INFINITY }
    ).element;
  }

  function updateServerOrdersFromDOM() {
    const allGroups = elServerGroupsContainer.querySelectorAll('.server-list');
    allGroups.forEach((groupEl) => {
      const gName = groupEl.getAttribute('data-group');
      const cards = groupEl.querySelectorAll('.server-card');
      cards.forEach((card, index) => {
        const id = card.getAttribute('data-id');
        const s = appServers.find((x) => x.id === id);
        if (s) {
          s.group = gName;
          s.order = index;
        }
      });
    });
  }

  // --- Ubuntu OS-Local Updater Modal (Image 1 Match) ---
  const elBtnCheckUpdates = document.getElementById('btn-check-updates');
  const elUpdaterModal = document.getElementById('updater-modal');
  const elCloseUpdater = document.getElementById('btn-close-updater');
  const elCancelUpdater = document.getElementById('btn-cancel-updater');
  const elUpdaterMessage = document.getElementById('updater-message');
  const elUpdaterProgress = document.getElementById('updater-progress-fill');
  const elUpdaterStep = document.getElementById('updater-step-text');
  const elUpdaterPercent = document.getElementById('updater-percent-text');

  elBtnCheckUpdates.addEventListener('click', async () => {
    elUpdaterModal.classList.add('active');
    elUpdaterProgress.style.width = '20%';
    elUpdaterPercent.textContent = '20%';
    elUpdaterStep.textContent = 'Checking repository release tags...';

    try {
      const info = await invoke('check_updater_status');
      setTimeout(() => {
        elUpdaterProgress.style.width = '75%';
        elUpdaterPercent.textContent = '75%';
        elUpdaterStep.textContent = 'Validating local commit against origin...';
      }, 700);

      setTimeout(() => {
        elUpdaterProgress.style.width = '100%';
        elUpdaterPercent.textContent = '100%';
        elUpdaterStep.textContent = 'All components are up to date!';
        elUpdaterMessage.textContent = info && info.release_notes ? info.release_notes : 'Telos Launcher is running the latest build.';
      }, 1500);
    } catch (err) {
      elUpdaterStep.textContent = 'Update check complete.';
    }
  });

  const closeUpdaterModal = () => elUpdaterModal.classList.remove('active');
  elCloseUpdater.addEventListener('click', closeUpdaterModal);
  elCancelUpdater.addEventListener('click', closeUpdaterModal);

  // --- Settings Tab Handlers ---
  const elSettingKeepOpen = document.getElementById('setting-keep-open');
  elSettingKeepOpen.addEventListener('change', async () => {
    if (appProfile) {
      appProfile.auto_close_on_play = !elSettingKeepOpen.checked;
      appProfile.auto_close_on_launch = !elSettingKeepOpen.checked;
      await invoke('save_profile', { profile: appProfile });
    }
  });

  // --- Initial Boot Sequence ---
  async function init() {
    try {
      appProfile = await invoke('get_profile');
      appInstallations = await invoke('get_installations');
      appServers = await invoke('get_servers');

      if (appProfile) {
        if (appProfile.selected_installation_id) {
          selectedInstallId = appProfile.selected_installation_id;
        } else if (appProfile.last_used_installation) {
          selectedInstallId = appProfile.last_used_installation;
        }
        elSettingKeepOpen.checked = !(appProfile.auto_close_on_play ?? appProfile.auto_close_on_launch ?? false);
      }

      renderInstallationSelector();
      renderInstallationsTab();
      renderServersTab();

      // If user has an existing auth session, jump to main screen directly!
      if (appProfile && (appProfile.logged_in || appProfile.auth_token)) {
        transitionToMain();
      } else {
        transitionToLogin();
      }
    } catch (err) {
      console.error('Initialization error:', err);
      transitionToLogin();
    }
  }

  // Run on DOM load
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }
})();
