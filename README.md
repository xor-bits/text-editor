<div align="center">

# text-editor

VIM/NeoVIM/Emacs Evil Mode/Helix/Kakoune -like cross-platform terminal based text editor

</div>

The plan is to create a text editor that can edit (almost) anything and (almost) anywhere.
Like a local config file, or a level.dat NBT file in a remote server over both SSH and Docker.

Remote editing should work like TRAMP in Emacs.

Buffers have a mode for the content type, like text, hex, NBT, or other binary formats.

![image](https://github.com/user-attachments/assets/76125b4c-1795-4a58-a3bd-ea58ffbc4408)

## Example usage

```
# just a normal local file
text-editor src/main.rs

# use sudo to edit /etc/fstab
text-editor sudo:/etc/fstab

# connect to user1@host1 using ssh, then connect to user2@host2 from host1 and open 'file'
text-editor ssh:user1@host1|ssh:user2@host2:file
```

## TODOs

 - [x] file editing
 - [x] remote (ssh/sudo/..) file editing
 - [x] multiple buffers
 - [x] file picker
 - [ ] hex editor
 - [ ] NBT editor
 - [ ] text-editor configuration
 - [ ] syntax highlighting
 - [ ] LSP
