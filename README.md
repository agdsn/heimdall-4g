# Heimdall

It's simply an SMS to mail gateway, with the possibility to specify routing of SMS.
The basic idea behind Heimdall is to forward 2FA tokens to a mailing list or certain persons, in order to allow access to shared accounts for the AG DSN.

## Hardware

Heimdall runs on a Pi Zero 2 with a PoE hat for powering the device and a Quectel BG96.
This low cost hardware is totally enough.
Just don't forget to add a fan, otherwise you get stuck when fetching network information.

## Notes

These are some side nodes before you start working with a deplyed setup.
First things first you definitifly should check rather the process *ModemManager* is not running on the system otherwise you will get glitchy behaviour.
Also node that you should definitfly add a rule into `/etc/udev/rule/99-modem.rules` in order to have a never changing device folder.
Could look like this:

```bash
SUBSYSTEM=="tty", ATTRS{idVendor}=="2c7c", ATTRS{idProduct}=="0296", ENV{ID_USB_INTERFACE_NUM}=="03", SYMLINK+="modem"
```

## Backends

Heimdall allows two different kinds of backends: one forwarding all SMS to one mail address, the other allows forwarding via an SQLite table, first checking whether an entry can be found and then sending the mail to the relevant mail address.

These can be configured via environment variables. Therefore put the needed config into `/var/heimdall/heimdall.env`.

## Installing Heimdall

Just download the *.deb* and run `sudo apt install <FILENAME>`.
The systemd service created can then be enabled in order to restart the service when needed.

I used an Ansible role for easy deployment and updating, looking like this:

```Ansible
---
- name: stop and disable a service
  ansible.builtin.service:
    name: "ModemManager"
    state: stopped
    enabled: false
- name: Download GitHub repository as deb
  ansible.builtin.get_url:
    url: "{{ repo_zip_url }}{{ deb_file }}"
    dest: "/tmp/"
    mode: "750"
- name: Start and enable heimdal
  ansible.builtin.service:
    name: "heimdall"
    state: stopped
    enabled: false
  when: "'heimdall.service' in ansible_facts.services"
- name: Install deb files
  ansible.builtin.apt:
    deb: "/tmp/{{ deb_file }}"
- name: create dir for heimdal
  ansible.builtin.file:
    path: "/var/heimdall/"
    state: directory
    mode: "750"
- name: Copy env-vars
  ansible.builtin.copy:
    src: heimdall/heimdall.env
    dest: /var/heimdall/heimdall.env
    decrypt: true
    mode: '0600'
- name: Start and enable heimdal
  ansible.builtin.service:
    name: "heimdall"
    state: started
    enabled: true
- name: Remove heimdall deb file
  ansible.builtin.file:
    path: "/tmp/{{ deb_file }}"
    state: absent
```
