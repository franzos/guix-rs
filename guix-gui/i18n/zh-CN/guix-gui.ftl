app-title = Guix GUI

tab-home = 主页
tab-search = 搜索
tab-installed = 已安装
tab-updates = 更新
tab-channels = 频道
tab-system = 设置
tab-about = 关于

settings-language = 语言
settings-language-system = 系统默认

# -- shared --
common-refresh = 刷新
common-cancel = 取消
common-close = 关闭
common-dismiss = 忽略
common-remove = 删除
common-open-settings = 打开设置

# -- app shell / overlay --
app-discover-failed = 无法发现 guix
app-discovering = 正在发现 guix……
app-brand = Guix
app-lightbox-close = 关闭（Esc）
app-lightbox-no-image = 无图片 / 加载失败
app-running = 正在运行……
app-copy = 复制
app-show-log = 显示日志
app-hide-log = 隐藏日志
# Status label shown in the progress overlay.
app-done = 完成。
app-failed-exit = 失败（退出码 { $code }）。
app-ended-no-summary = 结束时无退出摘要。
app-op-title = { $label }（操作 #{ $id }）
app-cancel-pkexec-tooltip = 无法取消特权操作——内核不允许向 root 拥有的进程发送信号。请等待其完成。
app-bootstrap-help =
    重新配置失败：正在运行的系统 Guix 无法识别你的配置所导入的某个模块。这通常意味着某个频道在你上次重新配置之后有了更新，而新模块尚未编入系统的 Guix 中。

    手动引导一次：

        sudo guix system reconfigure -L { $load } { $cfg }

    之后，此按钮即可用于后续的更新。
app-set-source-config-first = 请先在“设置”标签页中设置源配置路径。
app-op-start-failed = 启动操作失败：{ $error }
app-restore-panicked = 还原任务崩溃：{ $detail }
app-restore-failed = 还原失败：{ $detail }
app-discovery-client-failed = 发现客户端失败：{ $detail }

# -- op kinds --
op-install = 正在安装
op-remove = 正在删除
op-upgrade = 正在升级用户软件包
op-pull = 正在获取用户目录
op-system-pull = 正在获取系统目录
op-reconfigure = 正在重新配置系统

# -- stages --
stage-starting = 启动中
stage-channel-update = 正在更新频道
stage-computing-deriv = 正在计算衍生
stage-downloading = 正在下载
stage-building = 正在构建
stage-profile = 正在更新配置文件
# Status label shown in the progress overlay.
stage-done = 完成
# Status label shown in the progress overlay.
stage-failed = 失败
stage-build-failed = 构建失败：{ $name }
stage-build-failed-log = 构建失败：{ $name }（日志：{ $log }）

# -- categories --
category-graphics = 图形
category-audio-video = 音视频
category-office = 办公
category-development = 开发
category-engineering = 工程
category-internet = 互联网
category-game = 游戏

# -- home --
home-title = 主页
home-subtitle = 一个起点——Guix 中常见的应用程序。打开一个即可安装，或使用“搜索”浏览完整目录。
home-installed-badge = 已安装

# -- search --
search-title = 搜索
search-placeholder = 搜索软件包……
search-loading-catalog = 正在加载软件包目录……
search-searching = 正在搜索……
search-results = { $count } 个结果
search-truncated = 显示前 { $shown } 个，共 ≥{ $total } 个匹配项；请细化你的查询。
search-error-label = 搜索错误：
search-copy-details = 复制详情
search-select-prompt = 选择一个软件包以查看详情。
search-homepage = 主页：
search-license = 许可证：{ $license }
search-outputs = 输出：{ $outputs }
search-install = 安装
search-remove = 删除
search-screenshots-flathub = 截图来自 Flathub（{ $id }）
search-screenshots-debian = 截图来自 screenshots.debian.net
search-loading-media = 正在加载图标 / 截图……
search-failed = 搜索失败。

# -- installed --
installed-title = 已安装
installed-count = { $count } 个已安装软件包
installed-loading = 正在加载……
installed-error = 错误：{ $error }

# -- updates --
updates-title = 更新
updates-your-packages-blurb = 管理你的用户级软件包。
updates-your-packages = 你的软件包
updates-fetch-latest = 获取最新目录
updates-update-my-packages = 更新我的软件包
updates-system-blurb = 应用你的系统配置。需要管理员身份验证。
updates-system = 系统
updates-source-config = 源配置：{ $path }
updates-source-config-unset = 源配置：（未设置——打开“设置”进行选择）
updates-fetch-system = 获取系统目录
updates-update-system = 更新系统
# DO NOT TRANSLATE — literal shell command shown as a tooltip
updates-update-system-tip = pkexec guix system reconfigure
updates-confirm-reconfigure = 确认系统重新配置
updates-reconfigure-blurb = 通过 pkexec 以 root 身份运行。请核对下面的路径——每个路径都将由 Guile 以 root 权限加载。
updates-config = 配置：
updates-load-paths-none = 加载路径（-L）：（无）
updates-load-paths = 加载路径（-L），{ $count } 项：
updates-confirm-reconfigure-btn = 确认重新配置
updates-loading-channels = 正在加载频道……
updates-error-channels = 加载频道出错：{ $error }
updates-last-pulled = 上次拉取：{ $age }。
updates-last-pulled-never = 上次拉取：从未。
updates-channels-none = 频道：（未发现）。
updates-channels = 频道：{ $list }。
updates-channel-no-commit = （无提交）
updates-last-pulled-root = 上次拉取（root）：{ $age }。
updates-last-pulled-root-never = 上次拉取（root）：从未。
updates-last-reconfigured = 上次重新配置：{ $age }。
updates-last-reconfigured-never = 上次重新配置：从未（不是 Guix System 主机？）。

# -- about --
about-title = 关于
about-version = 版本 { $version }
about-tagline = Guix 软件包管理器的桌面前端。
about-authors = 作者
about-source = 源码与贡献
about-source-blurb = 欢迎提交 Bug 报告和拉取请求。
about-license = 许可证
about-license-line = Guix GUI 以 GNU 通用公共许可证 v3.0 发布。
about-license-detail = 你可以依据该许可证的条款重新分发和修改本软件。完整文本请参阅仓库中的 LICENSE 文件。
about-third-party = 第三方数据
about-third-party-blurb = 当你在“设置”中启用第三方元数据时，应用程序图标和截图会从外部服务获取。商标、图标和截图仍归各自项目所有。
about-channel-discovery = 频道发现
about-channel-discovery-blurb = “频道”标签页的“发现”子模式会浏览由 toys.whereis.social 索引的 Guix 频道和软件包。需选择启用；需要网络。该目录及其贡献者仍归各自项目所有。
about-built-with = 构建技术
about-built-with-detail = 各个 crate 的许可证列在其各自的仓库中。

# -- progress overlay --
progress-last = 最近：{ $line }
progress-running = 正在运行（{ $count }）：
progress-counts = 已构建 { $built }/{ $started }，已下载 { $dl_done }/{ $dl_started }（{ $mb } MB）
progress-build-line = { "  - " }{ $name } [building]
progress-build-item = { "  - " }{ $status ->
    [done] { $name } [done]
    [failed] { $name } [FAILED]
   *[other] { $name } [building]
}
progress-finished = 已完成（{ $done } 个完成，{ $failed } 个失败）：
progress-and-more = ……还有 { $count } 个
progress-active-downloads = 正在下载（{ $count }）：
progress-completed-downloads = 已完成下载（{ $count }）：
progress-starting = 正在启动……
progress-failed = 失败。
progress-stage-ellipsis = { $stage }……

# -- system / settings --
system-title = 设置
system-current-config = 当前系统配置：{ $path }
system-not-guix = 不在 Guix System 上：{ $error }
system-checking-config = 正在检查当前系统配置……
system-no-config-banner = 未在 /etc/config.scm 或 /etc/system.scm 检测到系统配置文件。请在下面输入你的 .scm 配置路径。
system-validate = 验证
system-config-heading = 系统配置
system-config-blurb = 你可编辑的 .scm 系统配置的路径。
system-config-placeholder = /home/you/dotfiles/config.scm
system-validation-empty = 路径为空。
system-validation-missing = 路径不存在：{ $path }
system-validation-not-file = 路径不是常规文件：{ $path }
system-validation-ok = 正常：{ $path }
system-load-paths-heading = 额外加载路径
system-load-paths-blurb = 解析 Scheme 导入时要搜索的额外目录。
system-load-paths-auto = 自动：{ $path }
system-load-paths-auto-unset = 自动：（请在上面设置系统配置）
system-load-paths-placeholder = /path/to/extra/modules
system-add = + 添加
system-section-system = SYSTEM
system-channels-heading = 频道
system-channels-blurb = 在专用标签页中管理用户级频道。
system-channels-none = 未配置频道。
system-channels-configured = 已配置 { $count } 个频道。
system-channels-unknown = 已配置频道：—
system-open-channels = 打开“频道”标签页
system-channels-source-heading = 用户频道源路径
system-channels-source-blurb = 覆盖 ~/.config/guix/channels.scm。当默认路径由 `guix home` 管理（解析到 /gnu/store）时为必填。
system-channels-source-placeholder = /home/you/dotfiles/channels.scm（留空使用默认值）
system-use-default = 使用默认值
system-section-user-channels = USER CHANNELS
system-metadata-heading = 图标与截图
system-metadata-blurb = 为选定的搜索结果从第三方目录获取图标和截图。需选择启用；需要网络访问。
system-metadata-enable = 启用第三方元数据
system-metadata-flathub = Flathub（flathub.org）
system-metadata-debian = screenshots.debian.net
system-cache-heading = 缓存
system-cache-blurb = 图标和截图会在磁盘上保存最多一年。如果某个图标在上游看起来有误，请清除缓存。
system-cache-dir = 缓存目录：{ $path }
system-cache-dir-none = 缓存目录：（未找到 XDG 缓存目录——仅使用内存）
system-clear-cache = 清除缓存
system-clearing-cache = 正在清除缓存……
system-cache-cleared = 缓存已清除。
system-cache-clear-failed = 清除缓存失败：{ $error }
system-discovery-heading = 发现
system-discovery-toggle = 浏览来自 toys.whereis.social 的频道和软件包
system-discovery-blurb = 需选择启用。需要网络访问。关闭时，发现功能不会出现在应用程序的任何位置。
system-desktop-refresh = 安装应用后刷新应用程序菜单
system-desktop-refresh-desc = 重建桌面菜单，让新安装的应用程序立即出现（KDE、XFCE、MATE、LXQt）。如果您更喜欢手动刷新，请关闭。
system-section-metadata = METADATA

# -- channels --
channels-title = 频道
channels-intro = 频道是 Guix 的软件包来源。添加一个频道可让你安装它提供的软件。删除一个频道意味着其软件包将不再获得更新。
channels-section-user = USER CHANNELS
channels-submode-installed = 已安装
channels-submode-discover = 发现
channels-default-path = ~/.config/guix/channels.scm（默认）
channels-store-managed = 由 store 管理（只读）
channels-writable = 可写
channels-file = 文件：{ $path }
channels-confirm-restore = 确认还原
channels-restore-last = 还原上次备份
channels-cant-edit-title = 无法在此处编辑该文件
channels-cant-edit-blurb = 你的 channels.scm 由 `guix home`（或其他工具）管理，无法直接编辑。若要使用 guix-gui 进行频道更改，请在 { $settings_tab } → { $channels_tab } 中设置一个可写文件。
channels-saving = 正在保存……
channels-pull-then-install = 拉取，然后安装 { $pkg }
channels-pull-only = 仅拉取
channels-pull-now = 立即拉取
channels-keep-changes = 保留更改
channels-pull-failed-shadow = 拉取失败——频道遮蔽 Bug（#74396）。
channels-pull-failed = 拉取失败。
channels-rollback-blurb = 你的 channels.scm 已有新更改，但 Guix 无法获取它们。是否还原之前的 channels.scm？
channels-rollback-none = 没有可还原的之前的 channels.scm。
channels-restore-previous = 还原之前的版本
channels-empty-title = 未找到 channels.scm
channels-empty-blurb = 在下面添加一个频道以创建它。该文件默认位于 ~/.config/guix/channels.scm。
channels-error = 错误
channels-loading = 正在加载 channels.scm……
channels-count = { $count } 个频道
channels-none-in-file = 此文件中没有频道。
channels-inherited-title = 也由你的频道引入
channels-inherited-blurb = 这些来自上面的频道并由它们管理。
channels-branch = 分支：{ $branch }
channels-commit = 提交：{ $commit }
channels-introduction = 引入：{ $fpr }
channels-no-fingerprint = （无指纹）
channels-introduction-none = 引入：（无）
channels-remove-title = 删除频道 `{ $name }`？
channels-remove-intro = 有 { $count } 个已安装软件包来自此频道：
channels-remove-explainer = 删除频道后，这些软件包仍可使用，但将不再接收更新。
channels-remove-cmd-label = 若要将它们与频道一并卸载：
channels-remove-anyway = 仍然删除频道
channels-locked = 已锁定
channels-see-warning = 见下方警告
channels-confirm-remove = 确认删除
channels-add-heading = 添加频道
channels-add-blurb = 所有字段均按原样存储；引入提交和指纹为必填项。
channels-add-name = 名称
channels-add-name-placeholder = 例如 nonguix
channels-add-url = URL
channels-add-url-placeholder = https://gitlab.com/nonguix/nonguix
channels-add-branch = 分支
channels-add-branch-placeholder = master（可选）
channels-add-commit = 提交
channels-add-commit-placeholder = 提交哈希（可选）
channels-add-intro-commit = 引入提交
channels-add-intro-commit-placeholder = 引入提交哈希
channels-add-intro-fpr = 引入指纹
channels-add-intro-fpr-placeholder = OpenPGP 指纹（例如 2A39 3FFF 68F4 EF7A 3D29 ...）
channels-add-btn = 添加频道
channels-discover-placeholder = 搜索软件包或频道……
channels-searching = 正在搜索……
channels-package-results = { $count } 个软件包结果
channels-from = 来自 { $channel }
channels-packages = 软件包
channels-channels-heading = 频道
channels-no-synopsis = （无简介）
channels-install = 安装
channels-add-and-install = 添加频道并安装
channels-loading-discover = 正在加载频道……
channels-no-introduced = 未返回任何已引入的频道。
channels-available = { $count } 个可用频道
channels-already-added = 已添加
channels-add = 添加
channels-pkgs = { $count } pkgs
channels-svcs = { $count } svcs
channels-intro-dash = intro: —
channels-intro-short = intro: { $fpr }...
channels-set-writable-tooltip = 在“设置”中设置一个可写文件
channels-confirm-add-title = 确认添加频道
channels-confirm-add-blurb = 这将把该频道追加到你的 channels.scm，并在保存前验证文件。
channels-provenance = 来源
channels-supplied-by = 由 { $source } 提供
channels-trust-warning = 添加后，每次 `guix pull` 都会以你的身份运行来自此来源的 Guile 代码。添加前，请将下面的引入提交和指纹与该频道自己公布的值核对。
channels-field-name = 名称
channels-field-url = url
channels-field-branch = 分支
channels-field-commit = 提交
channels-field-intro-commit = 引入提交
channels-field-intro-fpr = 引入指纹

# -- channels status messages --
channels-restored = 已从备份还原。
channels-added-install-prompt = 频道已添加。是否拉取，然后安装 { $pkg }？
channels-updated = 频道已更新。立即拉取以获取新目录。
channels-no-file-loaded = 未加载 channels.scm；请先刷新标签页。
channels-store-managed-error = { $path } 处的 channels.scm 由 store 管理。请在“设置”中设置一个可写的源路径覆盖。
channels-no-backup = 没有备份文件。
channels-form-name-required = 名称为必填项。
channels-form-url-required = URL 为必填项。
channels-form-intro-required = 引入提交和指纹为必填项。
channels-vanished-after-write = channels.scm 在写入后消失——请刷新标签页。
