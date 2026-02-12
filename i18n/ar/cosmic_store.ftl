app-name = متجر COSMIC
comment = متجر تطبيقات لسطح مكتب COSMIC
keywords = متجر;تطبيق;تطبيقات;برامج;
back = ارجع
cancel = ألغِ
check-for-updates = تحقق من وجود تحديثات
checking-for-updates = يتحقق مِن تحديثات...
close = أغلِق
install = نصِّب
no-installed-applications = لا توجد تطبيقات منصبة.
no-updates = جميع التطبيقات المنصبة محدثة.
no-results = لا توجد نتائج لـ ”{ $search }“.
notification-in-progress = عمليات التنصيب والتحديث جارية.
open = افتح
see-all = اعرض الكل
uninstall = ألغِ التنصيب
update = حدِّث
update-all = حدِّث الكل
place-on-desktop = ضع على سطح المكتب
place-applet = ضع بريمج
place-applet-desc = اختر مكان إضافة البريمج قبل تحديد موقعه بدقة.
panel = اللوحة
dock = المرسى
place-and-refine = ضع وحسِّن
# Codec dialog
codec-title = نصِّب حِزم الإضافية؟
codec-header = يتطلب «{ $application }» حِزمًا إضافية توفّر «{ $description }».
codec-footer =
    قد يكون استخدام هذه الحِزم الإضافية مقيدًا في بعض البلدان.
    يجب عليك التحقق من صحة أحد الأمور التالية:
     • لا تنطبق هذه القيود في بلد إقامتك القانونية
     • لديك إذن باستخدام هذا البرنامج (على سبيل المثال، ترخيص براءة اختراع)
     • أنت تستخدم هذا البرنامج لأغراض البحث فقط
codec-error = حدثت أخطاء أثناء تنصيب الحِزمة.
codec-installed = نُصِّبت الحِزم.
# Progress footer
details = التفاصيل
dismiss = أهمِل الرسالة
operations-running = { $running } عملية قيد التشغيل ({ $percent }٪)...
operations-running-finished = { $running } عملية قيد التشغيل ({ $percent }٪)، { $finished } انتهت...
# Repository add error dialog
repository-add-error-title = ”فشل في إضافة المستودع“
# Repository remove dialog
repository-remove-title = إزالة مستودع «{ $name }»؟
repository-remove-body =
    ستؤدي إزالة هذا المستودع إلى { $dependency ->
        [none] حذف
       *[other] إزالة «{ $dependency }» وحذف
    } التطبيقات والعناصر التالية. ستحتاج إلى إعادة تنصيبها إذا أُضيف المستودع مرة أخرى.
add = أضف
adding = يُضيف...
remove = أزِل
removing = يُزيل...
# Uninstall Dialog
uninstall-app = ألغِ تنصيب { $name }؟
uninstall-app-warning = سيؤدي إلغاء تنصيب { $name } إلى حذف بياناته.
# Nav Pages
explore = استكشف
create = أنشئ
work = اعمل
develop = تطوير
learn = تعلم
game = لعبة
relax = استرخِ
socialize = اجتمِع
utilities = الأدوات المساعدة
applets = بريمجات
installed-apps = التطبيقات المنصبة
updates = التحديثات

## Applets page

enable-flathub-cosmic = يرجى تفعيل Flathub و COSMIC Flatpak لرؤية البريمجات المتاحة.
manage-repositories = أدر المستودعات
# Explore Pages
editors-choice = اختيار المحرِر
popular-apps = التطبيقات الشائعة
made-for-cosmic = صنع من أجل COSMIC
new-apps = التطبيقات الجديدة
recently-updated = المحدثة حديثًا
development-tools = أدوات التطوير
scientific-tools = أدوات عِلمية
productivity-apps = تطبيقات إنتاجية
graphics-and-photography-tools = أدوات رسوميات وتصوير رقمي
social-networking-apps = تطبيقات التواصل الاجتماعي
games = ألعاب
music-and-video-apps = تطبيقات الموسيقى والفيديو
apps-for-learning = برامج للتعلم
# Details Page
source-installed = { $source } (نُصِّب)
developer = المطوِّر
app-developers = مطوري { $app }
monthly-downloads = تنزيلات Flathub الشهرية
licenses = التراخيص
proprietary = محتكرة

## App URLs

bug-tracker = متتبع العلل
contact = تواصل
donation = تبرع
faq = الأسئلة الشائعة
help = مساعدة
homepage = الصفحة الرئيسية
translate = ترجم

# Context Pages


## Operations

cancelled = أُلغِيَ
operations = العمليات
no-operations = لا توجد عمليات في التأريخ.
pending = قيد الانتظار
failed = فشل
complete = اكتمل

## Settings

settings = الإعدادات

## Release notes

latest-version = أحدث إصدار
no-description = لا يوجد وصف متاح.

## Repositories

recommended-flatpak-sources = مصادر فلاتباك الموصى بها
custom-flatpak-sources = مصادر فلاتباك مخصّصة
import-flatpakrepo = استورد ملف .flatpakrepo لإضافة مصدر مخصّص
no-custom-flatpak-sources = لا توجد مصادر فلاتباك مخصّصة
import = استورد
no-flatpak = لا دعم لِفلاتباك
software-repositories = مستودعات البرامج

### Appearance

appearance = المظهر
theme = النسق
match-desktop = طابق سطح المكتب
dark = داكن
light = فاتح
addons = الإضافات
view-more = اعرض المزيد
delete-app-data = احذف بيانات التطبيق نهائيًا
uninstall-app-flatpak-warning = إلغاء تنصيب { $name } سيحتفظ بمستنداته وبياناته.
version = الإصدار { $version }
system-package-updates = تحديثات الحِزم
system-packages-summary =
    { $count ->
        [one] { $count } حِزمة
       *[other] { $count } حِزمات
    }
system-packages = حِزم النظام
flatpak-runtimes = أزمنة تشغيل فلاتباك
