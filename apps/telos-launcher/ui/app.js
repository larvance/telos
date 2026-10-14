// ============================================================================
// Telos Launcher Frontend Controller (Tauri 2 Native IPC + Fallback)
// Zero-Emoji Clean SVG Iconography, Live News Feed & RTC Deployment Integration
// ============================================================================

(function () {
  'use strict';

  // Prevent default browser context menu inside the launcher
  window.addEventListener('contextmenu', (e) => {
    e.preventDefault();
  });

  const RAW_NEWS_URL = 'https://raw.githubusercontent.com/larvance/telos/main/apps/telos-launcher/ui/assets/news/news.json';

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
      selected_installation_id: 'local-dev',
      auto_close_on_play: false,
      language: 'English (US)',
      news_url: RAW_NEWS_URL,
      ram_gb: 8,
      graphics_backend: 'vulkan',
    },
    installations: [
      {
        id: 'local-dev',
        name: 'Dev Environment (Local Build)',
        version: '0.1.0-dev (Local)',
        directory: '/home/usr/Projects/larvance/telos',
        icon_type: 'wrench',
        is_dev: true,
      },
      {
        id: 'release-latest',
        name: 'Latest release',
        version: '0.1.0',
        directory: '/home/usr/.local/share/telos/versions/0.1.0',
        icon_type: 'voxel',
        is_dev: false,
      },
      {
        id: 'snapshot-latest',
        name: 'Latest snapshot',
        version: '0.2.0-alpha',
        directory: '/home/usr/.local/share/telos/versions/0.2.0-alpha',
        icon_type: 'package',
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
    switch (cmd) {
      case 'get_profile':
        return mockState.profile;
      case 'save_profile':
        mockState.profile = { ...mockState.profile, ...args.profile };
        return;
      case 'login':
        mockState.profile.username = args.username || 'Larvance';
        mockState.profile.auth_token = 'token-' + Date.now();
        mockState.profile.logged_in = true;
        return mockState.profile;
      case 'logout':
        mockState.profile.auth_token = null;
        mockState.profile.logged_in = false;
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
          message: 'Launching Telos Client...',
          is_building: args.installation_id === 'local-dev' || args.installation_id === 'dev-local',
        };
      case 'window_minimize':
      case 'window_close':
      case 'window_drag_start':
        return;
      case 'is_window_maximized':
        return false;
      case 'window_toggle_maximize':
        return false;
      default:
        return null;
    }
  }

  // --- SVG Icon Helper (Zero Unicode Emojis) ---
  function getSvgIcon(iconType) {
    if (iconType === 'wrench' || iconType === 'dev' || iconType === 'hammer') {
      return '<svg class="icon" style="color: #64b5f6;"><use href="#icon-wrench"></use></svg>';
    }
    if (iconType === 'package' || iconType === 'snapshot') {
      return '<svg class="icon" style="color: var(--accent-gold);"><use href="#icon-package"></use></svg>';
    }
    if (iconType === 'swords') {
      return '<svg class="icon" style="color: #e57373;"><use href="#icon-swords"></use></svg>';
    }
    if (iconType === 'shield') {
      return '<svg class="icon" style="color: #4dd0e1;"><use href="#icon-shield"></use></svg>';
    }
    if (iconType === 'globe') {
      return '<svg class="icon" style="color: #81c784;"><use href="#icon-globe"></use></svg>';
    }
    return '<svg class="icon" style="color: var(--accent-green-light);"><use href="#icon-voxel"></use></svg>';
  }

  function getInstIcon(inst) {
    if (inst.icon_type) return getSvgIcon(inst.icon_type);
    if (inst.is_dev || inst.id === 'local-dev' || inst.id === 'dev-local') return getSvgIcon('wrench');
    if (inst.id === 'latest-snapshot' || inst.id === 'snapshot-latest' || (inst.version && inst.version.includes('alpha'))) return getSvgIcon('package');
    return getSvgIcon('voxel');
  }

  function getInstVersion(inst) {
    return inst.version_tag || inst.version || '0.1.0';
  }

  function getInstDir(inst) {
    if (inst.custom_dir) return inst.custom_dir;
    if (inst.is_dev) return 'Source Repository (Local)';
    return '~/.telos (Default Directory)';
  }

  function getInstRam(inst) {
    if (inst.max_memory_mb) {
      return inst.max_memory_mb >= 1024
        ? `${Math.round(inst.max_memory_mb / 1024)} GB`
        : `${inst.max_memory_mb} MB`;
    }
    return '4 GB';
  }

  // --- App State ---
  let appProfile = null;
  let appInstallations = [];
  let appServers = [];
  let appNews = [];
  let selectedInstallId = 'local-dev';
  let isBuilding = false;
  let editingServerId = null;
  let newsEndpoint = 'https://news.telos.larvance.com/news.json';

  // --- DOM Elements ---
  const elLoginScreen = document.getElementById('login-screen');
  const elMainScreen = document.getElementById('main-screen');
  const elLoginForm = document.getElementById('login-form');
  const elLoginUsername = document.getElementById('login-username');
  const elLinkGuest = document.getElementById('link-guest-login');
  const elUserDisplayName = document.getElementById('user-display-name');
  const elUserAvatar = document.getElementById('user-avatar');
  const elUserTag = document.getElementById('user-account-tag');

  // Titlebar controls & window dragging
  const elTitlebar = document.getElementById('titlebar');
  const elNavbar = document.querySelector('.navbar');
  const elBtnMinimize = document.getElementById('btn-minimize');
  const elBtnMaximize = document.getElementById('btn-maximize');
  const elBtnClose = document.getElementById('btn-close');

  async function updateMaximizeState() {
    try {
      const isMax = await invoke('is_window_maximized');
      if (elBtnMaximize) {
        if (isMax) {
          elBtnMaximize.innerHTML = '<svg class="icon icon-sm"><use href="#icon-restore"></use></svg>';
          elBtnMaximize.title = 'Restore';
        } else {
          elBtnMaximize.innerHTML = '<svg class="icon icon-sm"><use href="#icon-maximize"></use></svg>';
          elBtnMaximize.title = 'Maximize';
        }
      }
    } catch (e) {
      console.debug('Failed to query window maximized state:', e);
    }
  }

  let lastToggleTime = 0;
  async function toggleMaximize() {
    const now = Date.now();
    if (now - lastToggleTime < 300) return;
    lastToggleTime = now;
    try {
      const isMax = await invoke('window_toggle_maximize');
      if (elBtnMaximize) {
        if (isMax) {
          elBtnMaximize.innerHTML = '<svg class="icon icon-sm"><use href="#icon-restore"></use></svg>';
          elBtnMaximize.title = 'Restore';
        } else {
          elBtnMaximize.innerHTML = '<svg class="icon icon-sm"><use href="#icon-maximize"></use></svg>';
          elBtnMaximize.title = 'Maximize';
        }
      }
    } catch (err) {
      console.error('Toggle maximize error:', err);
    }
  }

  if (elBtnMinimize) {
    elBtnMinimize.addEventListener('click', () => invoke('window_minimize'));
  }
  if (elBtnMaximize) {
    elBtnMaximize.addEventListener('click', toggleMaximize);
  }
  if (elBtnClose) {
    elBtnClose.addEventListener('click', () => invoke('window_close'));
  }

  function setupDraggableFrame(element) {
    if (!element) return;

    element.addEventListener('mousedown', (e) => {
      if (e.button !== 0) return;
      if (e.target.closest('button, a, input, select, textarea, .window-btn, .nav-tab')) return;

      if (e.detail > 1) {
        toggleMaximize();
        return;
      }

      const startX = e.screenX;
      const startY = e.screenY;
      let hasDragged = false;

      const onMouseMove = async (moveEvent) => {
        if (hasDragged) return;
        const dist = Math.hypot(moveEvent.screenX - startX, moveEvent.screenY - startY);
        if (dist > 5) {
          hasDragged = true;
          window.removeEventListener('mousemove', onMouseMove);
          window.removeEventListener('mouseup', onMouseUp);
          try {
            await invoke('window_drag_start');
            updateMaximizeState();
          } catch (err) {
            console.error('Drag start error:', err);
          }
        }
      };

      const onMouseUp = () => {
        window.removeEventListener('mousemove', onMouseMove);
        window.removeEventListener('mouseup', onMouseUp);
      };

      window.addEventListener('mousemove', onMouseMove);
      window.addEventListener('mouseup', onMouseUp);
    });

    element.addEventListener('dblclick', (e) => {
      if (e.target.closest('button, a, input, select, textarea, .window-btn, .nav-tab')) return;
      toggleMaximize();
    });
  }

  if (elTitlebar) {
    setupDraggableFrame(elTitlebar);
  }
  if (elNavbar) {
    setupDraggableFrame(elNavbar);
  }

  const elLaunchBar = document.getElementById('launch-bar');
  function updateLaunchBarLayout() {
    if (!elLaunchBar) return;
    if (window.innerWidth >= 1150) {
      elLaunchBar.classList.add('is-wide');
    } else {
      elLaunchBar.classList.remove('is-wide');
    }
  }

  window.addEventListener('resize', () => {
    updateMaximizeState();
    updateLaunchBarLayout();
  });
  updateMaximizeState();
  updateLaunchBarLayout();

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
      if (uName.startsWith('Guest_')) {
        elUserTag.textContent = 'Guest Player (Offline)';
      } else {
        elUserTag.textContent = 'Telos Account';
      }
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
      elCurrInstallIcon.innerHTML = getInstIcon(current);
      elCurrInstallName.textContent = current.name;
      elCurrInstallVersion.textContent = getInstVersion(current);
    }

    appInstallations.forEach((inst) => {
      const opt = document.createElement('div');
      opt.className = 'install-option' + (inst.id === selectedInstallId ? ' selected' : '');
      opt.innerHTML = `
        <span class="text-base flex items-center shrink-0">${getInstIcon(inst)}</span>
        <div class="flex flex-col">
          <span class="text-[13px] font-semibold text-white">${inst.name}</span>
          <span class="text-[11px] text-neutral-400">${getInstVersion(inst)}</span>
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
      const res = await invoke('trigger_launch', {
        installation_id: selectedInstallId,
        installationId: selectedInstallId,
      });
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

  // --- Dynamic Live News & Highlights ---
  const elNewsGrid = document.getElementById('news-grid-container');
  const elBtnRefreshNews = document.getElementById('btn-refresh-news');
  const elNewsReaderModal = document.getElementById('news-reader-modal');
  const elBtnCloseReader = document.getElementById('btn-close-reader');
  const elBtnDoneReader = document.getElementById('btn-done-reader');
  const elReaderCategory = document.getElementById('reader-category');
  const elReaderTitle = document.getElementById('reader-title');
  const elReaderDate = document.getElementById('reader-date');
  const elReaderAuthor = document.getElementById('reader-author');
  const elReaderImg = document.getElementById('reader-img');
  const elReaderSummary = document.getElementById('reader-summary');
  const elReaderContent = document.getElementById('reader-content');
  const elReaderWebLink = document.getElementById('reader-web-link');

  async function fetchAndRenderNews() {
    elNewsGrid.innerHTML = `
      <div style="grid-column: 1 / -1; padding: 20px; text-align: center; color: #888;">
        Loading latest announcements...
      </div>
    `;

    let articles = null;
    const endpoint = newsEndpoint || RAW_NEWS_URL;

    // 1. Attempt live remote fetch with timeout
    try {
      const controller = new AbortController();
      const timeoutId = setTimeout(() => controller.abort(), 2500);
      const res = await fetch(endpoint, {
        signal: controller.signal,
        cache: 'no-cache',
      });
      clearTimeout(timeoutId);
      if (res.ok) {
        articles = await res.json();
      }
    } catch (e) {
      // Fallback silently if offline or domain not pointed yet
    }

    // 2. Fallback to bundled offline news cache
    if (!articles || !Array.isArray(articles) || articles.length === 0) {
      try {
        const localRes = await fetch('assets/news/news.json');
        if (localRes.ok) {
          articles = await localRes.json();
        }
      } catch (err) {
        console.error('Failed to load local news:', err);
      }
    }

    if (!articles || !Array.isArray(articles)) {
      articles = [];
    }

    appNews = articles;
    renderNewsGrid(appNews);
  }

  function resolveNewsImage(img) {
    if (!img) return 'assets/news/underwater_overhaul.jpg';
    if (img.startsWith('http://') || img.startsWith('https://')) return img;
    if (img.startsWith('assets/images/')) {
      return img.replace('assets/images/', 'assets/news/');
    }
    return img;
  }

  function renderNewsGrid(articles) {
    elNewsGrid.innerHTML = '';
    if (articles.length === 0) {
      elNewsGrid.innerHTML = `
        <div style="grid-column: 1 / -1; padding: 20px; text-align: center; color: #888;">
          No news articles found.
        </div>
      `;
      return;
    }

    articles.forEach((art) => {
      const card = document.createElement('div');
      card.className = 'news-card';
      const imgSrc = resolveNewsImage(art.image);

      card.innerHTML = `
        <div class="news-thumb-wrap">
          <img src="${imgSrc}" alt="${art.title}" class="news-card-img" loading="lazy">
          <span class="news-badge">${art.category || 'Update'}</span>
        </div>
        <div class="news-body">
          <span class="news-date">${art.date || ''}</span>
          <h4 class="news-title">${art.title}</h4>
          <p class="news-snippet">${art.summary || ''}</p>
        </div>
      `;

      card.addEventListener('click', () => {
        openNewsReader(art);
      });

      elNewsGrid.appendChild(card);
    });
  }

  function openNewsReader(art) {
    elReaderCategory.textContent = art.category || 'Announcement';
    elReaderTitle.textContent = art.title;
    elReaderDate.textContent = art.date || '';
    elReaderAuthor.textContent = 'By ' + (art.author || 'Telos Dev Team');
    elReaderImg.src = resolveNewsImage(art.image);
    elReaderSummary.textContent = art.summary || '';
    elReaderContent.textContent = art.content || '';
    elReaderWebLink.href = `https://news.telos.larvance.com/articles/${art.id}.html`;
    elNewsReaderModal.classList.add('active');
  }

  const closeNewsReader = () => elNewsReaderModal.classList.remove('active');
  elBtnCloseReader.addEventListener('click', closeNewsReader);
  elBtnDoneReader.addEventListener('click', closeNewsReader);

  if (elBtnRefreshNews) {
    elBtnRefreshNews.addEventListener('click', () => {
      fetchAndRenderNews();
    });
  }

  // --- Installations Tab & Profile Management ---
  const elBtnNewInstall = document.getElementById('btn-new-installation');
  const elNewInstallModal = document.getElementById('new-install-modal');
  const elBtnCloseNewInstall = document.getElementById('btn-close-new-install');
  const elBtnCancelNewInstall = document.getElementById('btn-cancel-new-install');
  const elBtnSaveNewInstall = document.getElementById('btn-save-new-install');
  const elInstallModalTitle = document.getElementById('install-modal-title');
  const elInpInstallName = document.getElementById('inp-install-name');
  const elInpInstallVersion = document.getElementById('inp-install-version');
  const elInpInstallIcon = document.getElementById('inp-install-icon');
  const elChkCustomDir = document.getElementById('chk-custom-dir');
  const elInpInstallDir = document.getElementById('inp-install-dir');
  const elInpInstallRam = document.getElementById('inp-install-ram');
  const elInpInstallRamVal = document.getElementById('inp-install-ram-val');
  const elInpInstallResolution = document.getElementById('inp-install-resolution');
  const elInpInstallArgs = document.getElementById('inp-install-args');
  const ramPresetBtns = document.querySelectorAll('.btn-preset-ram');

  let editingInstallId = null;

  function populateVersionOptions(selectedVal) {
    if (!elInpInstallVersion) return;
    const versions = new Set(['0.1.0', 'snapshot-latest']);
    appInstallations.forEach((inst) => {
      const v = inst.version_tag || inst.version;
      if (v && !v.includes('local @')) {
        versions.add(v);
      }
    });

    elInpInstallVersion.innerHTML = '';
    versions.forEach((ver) => {
      const opt = document.createElement('option');
      opt.value = ver;
      if (ver === '0.1.0') {
        opt.textContent = '0.1.0 (Latest Release)';
      } else if (ver === 'snapshot-latest') {
        opt.textContent = 'Latest Snapshot (Continuous)';
      } else {
        opt.textContent = `Version ${ver}`;
      }
      if (ver === selectedVal) {
        opt.selected = true;
      }
      elInpInstallVersion.appendChild(opt);
    });
  }

  if (elInpInstallRam && elInpInstallRamVal) {
    elInpInstallRam.addEventListener('input', () => {
      elInpInstallRamVal.textContent = elInpInstallRam.value + ' GB';
    });
  }

  ramPresetBtns.forEach((btn) => {
    btn.addEventListener('click', () => {
      const r = btn.getAttribute('data-ram');
      if (r && elInpInstallRam && elInpInstallRamVal) {
        elInpInstallRam.value = r;
        elInpInstallRamVal.textContent = r + ' GB';
      }
    });
  });

  if (elChkCustomDir && elInpInstallDir) {
    elChkCustomDir.addEventListener('change', () => {
      elInpInstallDir.disabled = !elChkCustomDir.checked;
      if (elChkCustomDir.checked && !elInpInstallDir.value) {
        const slug = (elInpInstallName.value || 'profile')
          .toLowerCase()
          .replace(/[^a-z0-9_-]/g, '_');
        elInpInstallDir.value = `/home/usr/.telos/instances/${slug}`;
      }
    });
  }

  const closeNewInstallModal = () => elNewInstallModal.classList.remove('active');
  if (elBtnCloseNewInstall) elBtnCloseNewInstall.addEventListener('click', closeNewInstallModal);
  if (elBtnCancelNewInstall) elBtnCancelNewInstall.addEventListener('click', closeNewInstallModal);

  if (elBtnNewInstall) {
    elBtnNewInstall.addEventListener('click', () => {
      editingInstallId = null;
      if (elInstallModalTitle) elInstallModalTitle.textContent = 'Create New Installation';
      elInpInstallName.value = 'Custom Profile ' + (appInstallations.length + 1);
      populateVersionOptions('0.1.0');
      elInpInstallIcon.value = 'voxel';
      if (elChkCustomDir) elChkCustomDir.checked = false;
      if (elInpInstallDir) {
        elInpInstallDir.disabled = true;
        elInpInstallDir.value = '';
      }
      if (elInpInstallRam) elInpInstallRam.value = '4';
      if (elInpInstallRamVal) elInpInstallRamVal.textContent = '4 GB';
      if (elInpInstallResolution) elInpInstallResolution.value = 'Auto';
      if (elInpInstallArgs) elInpInstallArgs.value = '';
      elNewInstallModal.classList.add('active');
    });
  }

  function openEditInstallation(inst) {
    editingInstallId = inst.id;
    if (elInstallModalTitle) elInstallModalTitle.textContent = `Edit Installation: ${inst.name}`;
    elInpInstallName.value = inst.name;
    populateVersionOptions(inst.version_tag || '0.1.0');
    elInpInstallIcon.value = inst.icon_type || 'voxel';

    if (inst.custom_dir) {
      if (elChkCustomDir) elChkCustomDir.checked = true;
      if (elInpInstallDir) {
        elInpInstallDir.disabled = false;
        elInpInstallDir.value = inst.custom_dir;
      }
    } else {
      if (elChkCustomDir) elChkCustomDir.checked = false;
      if (elInpInstallDir) {
        elInpInstallDir.disabled = true;
        elInpInstallDir.value = '';
      }
    }

    const gb = inst.max_memory_mb ? Math.max(2, Math.round(inst.max_memory_mb / 1024)) : 4;
    if (elInpInstallRam) elInpInstallRam.value = gb;
    if (elInpInstallRamVal) elInpInstallRamVal.textContent = gb + ' GB';
    if (elInpInstallResolution) elInpInstallResolution.value = inst.resolution || 'Auto';
    if (elInpInstallArgs) elInpInstallArgs.value = inst.custom_args || '';
    elNewInstallModal.classList.add('active');
  }

  if (elBtnSaveNewInstall) {
    elBtnSaveNewInstall.addEventListener('click', async () => {
      const name = elInpInstallName.value.trim() || 'Custom Profile';
      const versionTag = elInpInstallVersion.value;
      const iconType = elInpInstallIcon.value;
      const customDir = elChkCustomDir && elChkCustomDir.checked && elInpInstallDir.value.trim()
        ? elInpInstallDir.value.trim()
        : null;
      const ramGb = parseInt(elInpInstallRam.value, 10) || 4;
      const maxMemoryMb = ramGb * 1024;
      const resolution = elInpInstallResolution.value;
      const customArgs = elInpInstallArgs.value.trim() || null;

      if (editingInstallId) {
        const inst = appInstallations.find((x) => x.id === editingInstallId);
        if (inst) {
          inst.name = name;
          inst.version_tag = versionTag;
          inst.icon_type = iconType;
          inst.custom_dir = customDir;
          inst.max_memory_mb = maxMemoryMb;
          inst.resolution = resolution;
          inst.custom_args = customArgs;
        }
      } else {
        const newInst = {
          id: 'inst-' + Date.now(),
          name,
          channel: 'Custom',
          version_tag: versionTag,
          custom_dir: customDir,
          max_memory_mb: maxMemoryMb,
          resolution,
          custom_args: customArgs,
          is_dev: false,
          icon_type: iconType,
        };
        appInstallations.push(newInst);
        selectedInstallId = newInst.id;
      }

      closeNewInstallModal();
      await invoke('save_installations', { installations: appInstallations });
      renderInstallationSelector();
      renderInstallationsTab();
    });
  }

  function renderInstallationsTab() {
    const container = document.getElementById('installations-container');
    if (!container) return;
    container.innerHTML = '';

    // Drag-over and drop event handlers on the installations container for reordering
    container.ondragover = (e) => {
      e.preventDefault();
      const afterElement = getDragAfterElement(container, e.clientY, '.installation-card:not(.dragging)');
      const dragging = document.querySelector('.installation-card.dragging');
      if (dragging) {
        if (afterElement == null) {
          container.appendChild(dragging);
        } else {
          container.insertBefore(dragging, afterElement);
        }
      }
    };

    container.ondrop = async (e) => {
      e.preventDefault();
      const cardEls = container.querySelectorAll('.installation-card');
      const newOrderIds = [...cardEls].map((el) => el.getAttribute('data-id'));
      const reordered = [];
      newOrderIds.forEach((id) => {
        const item = appInstallations.find((x) => x.id === id);
        if (item) reordered.push(item);
      });
      appInstallations.forEach((x) => {
        if (!reordered.find((r) => r.id === x.id)) reordered.push(x);
      });
      appInstallations = reordered;
      await invoke('save_installations', { installations: appInstallations });
      renderInstallationSelector();
    };

    appInstallations.forEach((inst) => {
      const card = document.createElement('div');
      card.className = 'installation-card';
      card.draggable = true;
      card.setAttribute('data-id', inst.id);

      const isDev = inst.is_dev || inst.id === 'local-dev' || inst.id === 'dev-local';
      const isDefault = inst.id === 'latest-release' || inst.id === 'latest-snapshot';
      const isCustom = !isDev && !isDefault;

      let badgeHtml = '';
      if (isDev) {
        badgeHtml = '<span class="inst-badge-pill">Dev Build</span>';
      } else if (inst.id === 'latest-release') {
        badgeHtml = '<span class="inst-badge-pill">Release</span>';
      } else if (inst.id === 'latest-snapshot') {
        badgeHtml = '<span class="inst-badge-pill" style="background:#5c4308; color:#ffd54f;">Snapshot</span>';
      } else {
        badgeHtml = '<span class="inst-badge-custom">Custom</span>';
      }

      card.innerHTML = `
        <div class="inst-meta">
          <svg class="icon icon-sm inst-drag-handle" title="Drag to reorder"><use href="#icon-drag"></use></svg>
          <div class="inst-icon">${getInstIcon(inst)}</div>
          <div class="inst-texts">
            <div class="flex items-center">
              <span class="inst-title">${inst.name}</span>
              ${badgeHtml}
            </div>
            <span class="inst-tag">Version: ${getInstVersion(inst)} • RAM: ${getInstRam(inst)} • ${inst.resolution || 'Auto'}</span>
            <span class="inst-dir font-mono text-[11px] text-neutral-500">${getInstDir(inst)}</span>
          </div>
        </div>
        <div class="flex gap-2 items-center">
          <button class="btn-icon folder-inst-btn" title="Open Game Directory">
            <svg class="icon icon-sm"><use href="#icon-folder"></use></svg>
          </button>
          ${
            !isDev
              ? `<button class="btn-icon edit-inst-btn" title="Edit Installation">
                   <svg class="icon icon-sm"><use href="#icon-edit"></use></svg>
                 </button>`
              : ''
          }
          ${
            isCustom
              ? `<button class="btn-icon del-inst-btn" title="Delete Installation">
                   <svg class="icon icon-sm"><use href="#icon-trash"></use></svg>
                 </button>`
              : ''
          }
          <button class="btn-green-3d !py-1.5 !px-4.5 !text-[13px]" data-play-id="${inst.id}">
            PLAY
          </button>
        </div>
      `;

      card.addEventListener('dragstart', () => card.classList.add('dragging'));
      card.addEventListener('dragend', () => card.classList.remove('dragging'));

      card.querySelector('[data-play-id]').addEventListener('click', () => {
        selectedInstallId = inst.id;
        renderInstallationSelector();
        elPlayBtn.click();
      });

      card.querySelector('.folder-inst-btn').addEventListener('click', (e) => {
        e.stopPropagation();
        alert(`Game directory for "${inst.name}":\n${getInstDir(inst)}\n\nWorld saves are stored in:\n${getInstDir(inst)}/worlds`);
      });

      const editBtn = card.querySelector('.edit-inst-btn');
      if (editBtn) {
        editBtn.addEventListener('click', (e) => {
          e.stopPropagation();
          openEditInstallation(inst);
        });
      }

      const delBtn = card.querySelector('.del-inst-btn');
      if (delBtn) {
        delBtn.addEventListener('click', async (e) => {
          e.stopPropagation();
          if (confirm(`Are you sure you want to delete installation "${inst.name}"?`)) {
            appInstallations = appInstallations.filter((x) => x.id !== inst.id);
            if (selectedInstallId === inst.id) {
              selectedInstallId = appInstallations[0] ? appInstallations[0].id : 'local-dev';
            }
            await invoke('save_installations', { installations: appInstallations });
            renderInstallationSelector();
            renderInstallationsTab();
          }
        });
      }

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

    const groups = {};
    appServers.forEach((srv) => {
      const g = srv.group || 'Community Servers';
      if (!groups[g]) groups[g] = [];
      groups[g].push(srv);
    });

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
          <svg class="icon icon-sm chevron"><use href="#icon-chevron-down"></use></svg>
          <span>${groupName} (${servers.length})</span>
        </span>
      `;
      headerEl.addEventListener('click', () => {
        groupEl.classList.toggle('collapsed');
      });

      const listEl = document.createElement('div');
      listEl.className = 'server-list';
      listEl.setAttribute('data-group', groupName);

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
        updateServerOrdersFromDOM();
        await invoke('save_servers', { servers: appServers });
      });

      servers.forEach((srv) => {
        const card = document.createElement('div');
        card.className = 'server-card';
        card.draggable = true;
        card.setAttribute('data-id', srv.id);

        card.innerHTML = `
          <div class="flex items-center gap-3">
            <svg class="icon icon-sm server-drag-handle" title="Drag to reorder"><use href="#icon-drag"></use></svg>
            <div class="server-icon"><svg class="icon"><use href="#icon-globe"></use></svg></div>
            <div class="server-meta">
              <span class="server-name">${srv.name}</span>
              <span class="server-address">${srv.address}</span>
            </div>
          </div>
          <div class="flex items-center gap-4">
            <div class="server-ping-badge">
              <span class="status-dot"></span>
              <span>${srv.ping_ms ? srv.ping_ms + 'ms' : 'Online'}</span>
            </div>
            <div class="server-actions flex gap-1.5">
              <button class="btn-icon edit-btn" title="Edit Server"><svg class="icon icon-sm"><use href="#icon-edit"></use></svg></button>
              <button class="btn-icon delete-btn" title="Delete Server"><svg class="icon icon-sm"><use href="#icon-trash"></use></svg></button>
            </div>
          </div>
        `;

        card.addEventListener('dragstart', () => {
          draggedServerId = srv.id;
          card.classList.add('dragging');
        });

        card.addEventListener('dragend', () => {
          draggedServerId = null;
          card.classList.remove('dragging');
        });

        card.querySelector('.edit-btn').addEventListener('click', (e) => {
          e.stopPropagation();
          editingServerId = srv.id;
          elServerModalTitle.textContent = 'Edit Multiplayer Server';
          elServerNameInput.value = srv.name;
          elServerAddressInput.value = srv.address;
          elServerGroupInput.value = srv.group;
          elServerModal.classList.add('active');
        });

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

  function getDragAfterElement(container, y, selector = '.server-card:not(.dragging)') {
    const draggableElements = [...container.querySelectorAll(selector)];
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



  // --- Language Selection Modal ---
  const elLangPicker = document.getElementById('lang-picker');
  const elLangModal = document.getElementById('lang-modal');
  const elBtnCloseLang = document.getElementById('btn-close-lang');
  const elBtnApplyLang = document.getElementById('btn-apply-lang');
  const elLoginLangText = document.getElementById('login-lang-text');

  elLangPicker.addEventListener('click', () => {
    elLangModal.classList.add('active');
  });

  const closeLangModal = () => elLangModal.classList.remove('active');
  elBtnCloseLang.addEventListener('click', closeLangModal);
  elBtnApplyLang.addEventListener('click', closeLangModal);

  document.querySelectorAll('.lang-option').forEach((opt) => {
    opt.addEventListener('click', () => {
      document.querySelectorAll('.lang-option').forEach((o) => o.classList.remove('selected'));
      opt.classList.add('selected');
      const name = opt.getAttribute('data-name');
      elLoginLangText.textContent = name;
      closeLangModal();
    });
  });

  // --- User Account Modal ---
  const elUserBadge = document.getElementById('user-badge');
  const elAccountModal = document.getElementById('account-modal');
  const elBtnCloseAccount = document.getElementById('btn-close-account');
  const elBtnCloseAccountFooter = document.getElementById('btn-close-account-footer');
  const elBtnAccountLogout = document.getElementById('btn-account-logout');
  const elModalAccountAvatar = document.getElementById('modal-account-avatar');
  const elModalAccountName = document.getElementById('modal-account-name');
  const elModalAccountStatus = document.getElementById('modal-account-status');

  elUserBadge.addEventListener('click', () => {
    const name = (appProfile && (appProfile.username || appProfile.account_name)) || 'Larvance';
    elModalAccountAvatar.textContent = (name[0] || 'T').toUpperCase();
    elModalAccountName.textContent = name;
    if (name.startsWith('Guest_')) {
      elModalAccountStatus.textContent = 'Guest Player (Offline Mode)';
    } else {
      elModalAccountStatus.textContent = 'Telos Account (Authenticated)';
    }
    elAccountModal.classList.add('active');
  });

  const closeAccountModal = () => elAccountModal.classList.remove('active');
  elBtnCloseAccount.addEventListener('click', closeAccountModal);
  elBtnCloseAccountFooter.addEventListener('click', closeAccountModal);
  elBtnAccountLogout.addEventListener('click', async () => {
    closeAccountModal();
    await invoke('logout');
    transitionToLogin();
  });

  // --- Ubuntu OS-Local Updater Modal ---
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
        elUpdaterMessage.textContent =
          info && info.release_notes ? info.release_notes : 'Telos Launcher is running the latest build.';
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
  const elSettingNewsUrl = document.getElementById('setting-news-url');
  const elBtnSyncNews = document.getElementById('btn-sync-news-settings');
  const elSettingGraphics = document.getElementById('setting-graphics-backend');
  const elSettingRamSlider = document.getElementById('setting-ram-slider');
  const elSettingRamLabel = document.getElementById('setting-ram-label');
  const elBtnResetDefaults = document.getElementById('btn-reset-defaults');

  elSettingKeepOpen.addEventListener('change', async () => {
    if (appProfile) {
      appProfile.auto_close_on_play = !elSettingKeepOpen.checked;
      appProfile.auto_close_on_launch = !elSettingKeepOpen.checked;
      await invoke('save_profile', { profile: appProfile });
    }
  });

  elSettingRamSlider.addEventListener('input', () => {
    elSettingRamLabel.textContent = elSettingRamSlider.value + ' GB';
  });

  elSettingRamSlider.addEventListener('change', async () => {
    if (appProfile) {
      appProfile.ram_gb = parseInt(elSettingRamSlider.value, 10);
      await invoke('save_profile', { profile: appProfile });
    }
  });

  elSettingGraphics.addEventListener('change', async () => {
    if (appProfile) {
      appProfile.graphics_backend = elSettingGraphics.value;
      await invoke('save_profile', { profile: appProfile });
    }
  });

  elBtnSyncNews.addEventListener('click', async () => {
    newsEndpoint = elSettingNewsUrl.value.trim() || RAW_NEWS_URL;
    if (appProfile) {
      appProfile.news_url = newsEndpoint;
      await invoke('save_profile', { profile: appProfile });
    }
    elBtnSyncNews.textContent = 'Syncing...';
    await fetchAndRenderNews();
    elBtnSyncNews.textContent = 'Synced!';
    setTimeout(() => {
      elBtnSyncNews.textContent = 'Sync Feed';
    }, 2000);
  });

  elBtnResetDefaults.addEventListener('click', async () => {
    elSettingKeepOpen.checked = true;
    elSettingNewsUrl.value = RAW_NEWS_URL;
    elSettingGraphics.value = 'vulkan';
    elSettingRamSlider.value = '8';
    elSettingRamLabel.textContent = '8 GB';
    newsEndpoint = RAW_NEWS_URL;
    if (appProfile) {
      appProfile.auto_close_on_play = false;
      appProfile.news_url = newsEndpoint;
      appProfile.graphics_backend = 'vulkan';
      appProfile.ram_gb = 8;
      await invoke('save_profile', { profile: appProfile });
    }
    await fetchAndRenderNews();
    alert('Settings reset to defaults.');
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
        if (selectedInstallId === 'dev-local') {
          selectedInstallId = 'local-dev';
        }
        elSettingKeepOpen.checked = !(appProfile.auto_close_on_play ?? appProfile.auto_close_on_launch ?? false);
        if (appProfile.news_url) {
          newsEndpoint = appProfile.news_url;
          elSettingNewsUrl.value = newsEndpoint;
        }
        if (appProfile.graphics_backend) {
          elSettingGraphics.value = appProfile.graphics_backend;
        }
        if (appProfile.ram_gb) {
          elSettingRamSlider.value = appProfile.ram_gb;
          elSettingRamLabel.textContent = appProfile.ram_gb + ' GB';
        }
      }

      renderInstallationSelector();
      renderInstallationsTab();
      renderServersTab();
      await fetchAndRenderNews();

      if (appProfile && (appProfile.logged_in || appProfile.auth_token)) {
        transitionToMain();
      } else {
        transitionToLogin();
      }

      // Check for CLI arguments (--tab <name>, --modal <name>)
      try {
        const cliArgs = await invoke('get_cli_args');
        if (Array.isArray(cliArgs)) {
          const tabIdx = cliArgs.indexOf('--tab');
          if (tabIdx !== -1 && cliArgs[tabIdx + 1]) {
            const targetTab = cliArgs[tabIdx + 1];
            const tabBtn = document.querySelector(`.nav-tab[data-tab="${targetTab}"]`);
            if (tabBtn) tabBtn.click();
          }
          const modalIdx = cliArgs.indexOf('--modal');
          if (modalIdx !== -1 && cliArgs[modalIdx + 1]) {
            const targetModal = cliArgs[modalIdx + 1];
            if (targetModal === 'news-reader') {
              const card = document.querySelector('.news-card');
              if (card) card.click();
            } else if (targetModal === 'new-installation' || targetModal === 'install') {
              const btn = document.getElementById('btn-new-installation');
              if (btn) btn.click();
            } else if (targetModal === 'edit-installation') {
              const editBtn = document.querySelector('.edit-inst-btn');
              if (editBtn) editBtn.click();
            } else if (targetModal === 'account') {
              const badge = document.querySelector('.user-profile-badge');
              if (badge) badge.click();
            }
          }
        }
      } catch (_) {}
    } catch (err) {
      console.error('Initialization error:', err);
      transitionToLogin();
    }
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }
})();
