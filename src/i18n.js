/* Small bilingual (EN / 中文) engine for the Small Deck agent.
 * Mirrors the XPAD web pattern: window.lang / window.t / window.setLang,
 * [data-i18n] elements filled on apply(), language persisted to localStorage. */
(function () {
  var DICT = {
    app_sub:    { en: 'Small Deck',  zh: 'Small Deck' },
    link_home:  { en: 'XPAD Home',   zh: 'XPAD 主页' },
    link_docs:  { en: 'Docs',        zh: '文档' },
    intro:      { en: 'Bind the F13–F24 keys you assigned in the XPAD configurator to programs, files or links.',
                  zh: '把你在 XPAD 配置器里分配的 F13–F24 键绑定到程序、文件或链接。' },
    btn_read:   { en: 'Read XPAD',   zh: '读取 XPAD' },
    show_all:   { en: 'Show all F13–F24', zh: '显示全部 F13–F24' },
    autostart:  { en: 'Start on login',   zh: '开机自启' },
    settings:   { en: 'Settings',         zh: '设置' },
    language:   { en: 'Language',         zh: '语言' },
    tray_show:  { en: 'Show',             zh: '显示' },
    tray_quit:  { en: 'Quit',             zh: '退出' },
    quit_full:  { en: 'Quit completely',  zh: '彻底退出程序' },
    open_browser:{ en: 'Open in browser', zh: '用浏览器打开' },
    home_notice:{ en: 'For full functionality (USB device access), open this page in a Chromium-based browser such as Chrome or Edge.',
                  zh: '如需使用完整功能（USB 设备访问），请在 Chrome / Edge 等 Chromium 内核浏览器中打开本页。' },
    empty_hint: { en: 'No F13–F24 keys are mapped on the device yet. Open the XPAD configurator, assign keys on the “Small Deck” tab, write them to the device, then click “Read XPAD”.',
                  zh: '设备上还没有映射任何 F13–F24 键。打开 XPAD 配置器，在“Small Deck”页分配按键并写入设备，然后点击“读取 XPAD”。' },

    // dynamic (used from app.js via t())
    unset:      { en: 'Not set',     zh: '未设置' },
    unsupported:{ en: 'Not supported on this OS (no global-hotkey scancode)', zh: '本系统不支持（无全局热键扫描码）' },
    browse:     { en: 'Browse…',     zh: '浏览…' },
    edit:       { en: 'Edit',        zh: '编辑' },
    set_to:     { en: 'Set to:',     zh: '设置为：' },
    type_program:{ en: 'Program',    zh: '程序' },
    type_folder:{ en: 'Folder',      zh: '文件夹' },
    type_url:   { en: 'URL',         zh: '网址' },
    type_command:{ en: 'Command',    zh: '命令' },
    save:       { en: 'Save',        zh: '保存' },
    test:       { en: 'Test',        zh: '测试' },
    clear:      { en: 'Clear',       zh: '清除' },
    close:      { en: 'Close',        zh: '关闭' },
    done:       { en: 'Done',         zh: '完成' },
    matrix_view:{ en: 'Device matrix', zh: '设备按键矩阵' },
    list_view:  { en: 'List view',    zh: '列表视图' },
    seg_matrix: { en: 'Matrix',       zh: '矩阵' },
    seg_list:   { en: 'List',         zh: '列表' },
    empty_title:{ en: 'No device read yet', zh: '还没读取设备' },
    not_bundled:{ en: 'Not tied to XPAD — any keyboard that sends F13–F24 works.',
                  zh: '不绑定 XPAD，任何能发送 F13–F24 的键盘都可用。' },
    dev_none:   { en: 'No device',    zh: '未连接' },
    dev_ready:  { en: 'Connected · {n} key(s)', zh: '已连接 · {n} 个键' },
    dev_nokeys: { en: 'Connected · no Small Deck keys', zh: '已连接 · 无 Small Deck 键' },
    matrix_hint:{ en: 'Coloured keys are Small Deck (F13–F24) triggers — click one to bind a program. Grey keys are other keys.',
                  zh: '彩色键为 Small Deck（F13–F24）触发键，点击可绑定程序；灰色键为其它按键。' },
    reading:    { en: 'Reading XPAD…', zh: '正在读取 XPAD…' },
    read_ok:    { en: 'Read {n} Small Deck key(s) from the device.', zh: '已从设备读取 {n} 个 Small Deck 键。' },
    launch_ok:  { en: 'Launched.',   zh: '已启动。' },
    pick_title: { en: 'Choose a program or file', zh: '选择程序或文件' }
  };

  var lang = localStorage.getItem('xpad_lang') || 'en';

  function t(key, vars) {
    var e = DICT[key];
    var s = e ? (e[lang] || e.en) : key;
    if (vars) for (var k in vars) s = s.replace('{' + k + '}', vars[k]);
    return s;
  }

  function apply() {
    document.documentElement.lang = (lang === 'zh') ? 'zh-CN' : 'en';
    document.querySelectorAll('[data-i18n]').forEach(function (el) {
      var e = DICT[el.getAttribute('data-i18n')];
      if (e) el.textContent = e[lang] || e.en;
    });
    document.querySelectorAll('.lang-btn').forEach(function (b) {
      b.classList.toggle('active', b.getAttribute('data-lang') === lang);
    });
    if (typeof window.render === 'function') window.render();
    if (typeof window.syncTrayLabels === 'function') window.syncTrayLabels();
  }

  window.setLang = function (l) {
    lang = l;
    window.lang = l;
    localStorage.setItem('xpad_lang', l);
    apply();
  };
  window.t = t;
  window.lang = lang;
  window.addEventListener('DOMContentLoaded', apply);
})();
