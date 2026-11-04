import Foundation
import UserNotifications

/// Silences system audio output, regardless of its source. Un-muting is left entirely to the
/// person: there is no corresponding un-mute call anywhere in this app.
func muteSystemAudio() {
    let script = NSAppleScript(source: "set volume output muted true")
    var error: NSDictionary?
    script?.executeAndReturnError(&error)
}

/// Asks once for permission to show notifications. Safe to call repeatedly: the system only
/// prompts the first time.
func requestNotificationAuthorization() {
    UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound]) { _, _ in }
}

func postPrayerNotification(name: String) {
    let content = UNMutableNotificationContent()
    content.title = "Hya"
    content.body = "\(name) in 5 minutes — audio muted"
    let request = UNNotificationRequest(identifier: UUID().uuidString, content: content, trigger: nil)
    UNUserNotificationCenter.current().add(request)
}
