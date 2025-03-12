<div align="center">

# text-editor

VIM/NeoVIM/Emacs Evil Mode/Helix/Kakoune -like cross-platform terminal based text editor

</div>

The plan is to create a text editor that can edit (almost) anything and (almost) anywhere.
Like a local config file, or a level.dat NBT file in a remote server over both SSH and Docker.

Remote editing should work like TRAMP in Emacs.

Buffers have a mode for the content type, like text, hex, NBT, or other binary formats.

## Example usage

```
# just a normal local file
text-editor src/main.rs

# use sudo to edit /etc/fstab
text-editor sudo:/etc/fstab

# connect to user1@host1 using ssh, then connect to user2@host2 from host1 and open 'file'
text-editor ssh:user1@host1|ssh:user2@host2:file
```

## Screenshots

### normal file editing
![image](https://github.com/user-attachments/assets/76125b4c-1795-4a58-a3bd-ea58ffbc4408)

### file explorer and editor over ssh
![image](https://github.com/user-attachments/assets/15d27e2b-8e1b-4caf-877f-b7e16dafac07)

### command suggestions
![image](https://github.com/user-attachments/assets/cfeab306-0eee-4856-8741-55b1f02c6fbd)

## TODOs

 - [x] file editing
 - [x] remote (ssh/sudo/docker/..) file editing
 - [x] multiple buffers
 - [x] file picker
 - [x] buffer picker
 - [ ] SSH/sudo askpass somehow
 - [ ] hex editor
 - [ ] NBT editor
 - [ ] text-editor configuration
 - [ ] syntax highlighting
 - [ ] LSP
