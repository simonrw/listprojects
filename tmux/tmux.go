package tmux

import (
	"bytes"
	"fmt"
	"os"
	"os/exec"
	"strings"

	"gitlab.com/srwalker101/project/projectpath"
)

// Session wraps the functionality of tmux
type Session struct {
	path projectpath.Path
}

func NewSession(p projectpath.Path) *Session {
	return &Session{p}
}

func (s Session) Switch() error {
	if s.tmuxRunning() {
		exists, err := s.exists()
		if err != nil {
			return fmt.Errorf("checking if session exists: %w", err)
		}

		if exists {
			if err = s.switchClient(); err != nil {
				return fmt.Errorf("switching client: %w", err)
			}
		} else {
			if err = s.createSession(); err != nil {
				return fmt.Errorf("creating session: %w", err)
			}
			if err = s.switchClient(); err != nil {
				return fmt.Errorf("switching client: %w", err)
			}
		}
	} else {
		if err := s.createSession(); err != nil {
			return fmt.Errorf("creating session: %w", err)
		}
		if err := s.join(); err != nil {
			return fmt.Errorf("joining session: %w", err)
		}
	}

	return nil
}

func (s Session) tmuxRunning() bool {
	return os.Getenv("TMUX") != ""
}

func (s Session) exists() (bool, error) {
	var buf bytes.Buffer
	cmd := exec.Command("tmux", "ls", "-F", "#S")
	cmd.Stdout = &buf
	err := cmd.Run()
	if err != nil {
		return false, fmt.Errorf("error spawning command: %w", err)
	}

	output := strings.Split(buf.String(), "\n")
	for _, session := range output {
		if session == s.sessionName() {
			return true, nil
		}
	}
	return false, nil
}

func (s Session) switchClient() error {
	cmd := exec.Command("tmux", "switch-client", "-t", s.path.SessionName)
	return cmd.Run()
}

func (s Session) createSession() error {
	cmd := exec.Command("tmux", "new-session", "-d", "-c", s.path.FullPath, "-s", s.path.SessionName)
	return cmd.Run()
}

func (s Session) join() error {
	cmd := exec.Command("tmux", "attach-session", "-s", s.path.SessionName)
	return cmd.Run()
}

func (s Session) sessionName() string {
	return s.path.SessionName
}
