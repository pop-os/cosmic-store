app-name = COSMIC 软件商店
back = 返回
cancel = 取消
check-for-updates = 检查更新
checking-for-updates = 正在检查更新...
close = 关闭
install = 安装
no-installed-applications = 没有已安装的应用程序。
no-updates = 所有已安装的应用程序都是最新的。
no-results = 找不到 "{ $search }" 的结果。
notification-in-progress = 正在进行安装和更新。
open = 打开
see-all = 查看全部
uninstall = 卸载
update = 更新
update-all = 全部更新
place-on-desktop = 放置到桌面
place-applet = 放置小部件
place-applet-desc = 调整小部件位置前，请先选择添加小部件的位置。
panel = 面板
dock = 程序坞
place-and-refine = 放置并调整
# Codec dialog
codec-title = 安装额外的软件包？
codec-header = "{ $application }" 需要额外的软件包来提供 "{ $description }" 。
codec-footer =
    在某些国家，使用这些额外的软件包可能受到限制。
    您必须验证以下其中一项是否正确：
    ・这些限制不适用于您的合法居住国
    ・您有权利使用此软件（例如，专利许可证）
    ・您仅将此软件用于研究目的
codec-error = 软件包安装过程中出现错误。
codec-installed = 软件包已安装。
# Progress footer
details = 详情
dismiss = 清除消息
operations-running = { $running } 个操作正在运行（{ $percent }%）…
operations-running-finished = { $running } 个操作正在运行（{ $percent }%），{ $finished } 个已完成...
# Repository add error dialog
repository-add-error-title = "添加远程仓库失败"
# Repository remove dialog
repository-remove-title = 移除 "{ $name }" 远程仓库？
repository-remove-body =
    移除该远程仓库将 { $dependency ->
        [none] 删除
       *[other] 删除 "{ $dependency }" 以及
    } 以下应用程序。如果再次添加远程仓库，需要重新安装它们。
add = 添加
adding = 正在添加...
remove = 移除
removing = 正在移除...
# Uninstall Dialog
uninstall-app = 确定要卸载 { $name } ？
uninstall-app-warning = 卸载 { $name } 不会保留任何数据。
# Nav Pages
explore = 探索
create = 创作
work = 工作
develop = 开发
learn = 学习
game = 游戏
relax = 娱乐
socialize = 社交
utilities = 实用工具
applets = 小部件
installed-apps = 已安装的应用程序
updates = 更新

## Applets page

enable-flathub-cosmic = 请启用 Flathub 和 COSMIC Flatpak 远程仓库即可查看可用小部件。
manage-repositories = 管理远程仓库
# Explore Pages
editors-choice = 编辑推荐
popular-apps = 热门应用
made-for-cosmic = 为 COSMIC 设计
new-apps = 新应用程序
recently-updated = 最近更新
development-tools = 开发工具
scientific-tools = 科学工具
productivity-apps = 效率应用
graphics-and-photography-tools = 图形和设计工具
social-networking-apps = 社交网络应用
games = 游戏
music-and-video-apps = 音乐与视频应用
apps-for-learning = 学习应用
# Details Page
source-installed = { $source }（已安装）
developer = 开发者
app-developers = { $app } 开发者
monthly-downloads = Flathub 每月下载量
licenses = 许可证
proprietary = 专有

## App URLs

bug-tracker = 错误跟踪
contact = 联系
donation = 捐赠
faq = 常见问题
help = 帮助
homepage = 主页
translate = 翻译

# Context Pages


## Operations

cancelled = 已取消
operations = 操作
no-operations = 历史中没有任何操作。
pending = 待处理
failed = 失败
complete = 完成

## Settings

settings = 设置

## Release notes

latest-version = 最新版本
no-description = 没有可用的描述。

## Repositories

recommended-flatpak-sources = 推荐的 Flatpak 远程仓库
custom-flatpak-sources = 自定义 Flatpak 远程仓库
import-flatpakrepo = 导入 .flatpakrepo 文件即可添加自定义远程仓库
no-custom-flatpak-sources = 没有自定义 Flatpak 远程仓库
import = 导入
no-flatpak = 不支持 Flatpak
software-repositories = 软件仓库

### Appearance

appearance = 外观
theme = 主题
match-desktop = 匹配桌面
dark = 暗色模式
light = 亮色模式
addons = 插件
view-more = 查看更多
delete-app-data = 永久移除应用数据
uninstall-app-flatpak-warning = 卸载 { $name } 会保留所有文件和数据。
version = { $version } 版本
system-package-updates = 软件包更新
system-packages-summary =
    { $count ->
        [one] { $count } 个软件包
       *[other] { $count } 个软件包
    }
system-packages = 系统软件包
flatpak-runtimes = Flatpak 运行时
