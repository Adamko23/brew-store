#!/usr/bin/osascript
on run argv
    set reply to display dialog "Homebrew potrebuje oprávnenie správcu na dokončenie operácie spustenej v Brew Store." with title "Brew Store — Homebrew" default answer "" with hidden answer buttons {"Zrušiť", "Povoliť"} default button "Povoliť" cancel button "Zrušiť"
    return text returned of reply
end run
