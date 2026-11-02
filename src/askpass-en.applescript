#!/usr/bin/osascript
on run argv
    set reply to display dialog "Homebrew needs administrator access to finish the operation you started in Brew Store." with title "Brew Store — Homebrew" default answer "" with hidden answer buttons {"Cancel", "Allow"} default button "Allow" cancel button "Cancel"
    return text returned of reply
end run
