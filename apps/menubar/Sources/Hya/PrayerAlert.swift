import Foundation
import UserNotifications

func muteSystemAudio() {
    let script = NSAppleScript(source: "set volume output muted true")
    var error: NSDictionary?
    script?.executeAndReturnError(&error)
}

func requestNotificationAuthorization() {
    UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound]) { _, _ in }
}

func postPrayerNotification(name: String) {
    let content = UNMutableNotificationContent()
    content.title = "Hya"
    content.body = "\(name) in 5 minutes. Audio muted"
    let request = UNNotificationRequest(identifier: UUID().uuidString, content: content, trigger: nil)
    UNUserNotificationCenter.current().add(request)
}
