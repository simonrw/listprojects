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

func (s Session) runTmuxCommand(args ...string) (string, error) {
	var outBuf bytes.Buffer
	var errBuf bytes.Buffer
	cmd := exec.Command("tmux", args...)
	cmd.Stdout = &outBuf
	cmd.Stderr = &errBuf
	err := cmd.Run()
	if err != nil {
		return "", fmt.Errorf("running tmux command: %w (%s)", err, errBuf.String())
	}
	return outBuf.String(), nil
}

func (s Session) exists() (bool, error) {
	out, err := s.runTmuxCommand("ls", "-F", "#S")
	if err != nil {
		return false, fmt.Errorf("error spawning command: %w", err)
	}

	output := strings.Split(out, "\n")
	for _, session := range output {
		if session == s.sessionName() {
			return true, nil
		}
	}
	return false, nil
}

func (s Session) switchClient() error {
	_, err := s.runTmuxCommand("switch-client", "-t", s.sessionName())
	if err != nil {
		return fmt.Errorf("switching client: %w", err)
	}
	return nil
}

func (s Session) createSession() error {
	_, err := s.runTmuxCommand("new-session", "-d", "-c", s.path.FullPath, "-s", s.sessionName())
	if err != nil {
		return fmt.Errorf("creating new session: %w", err)
	}
	return nil
}

func (s Session) join() error {
	_, err := s.runTmuxCommand("attach-session", "-s", s.sessionName())
	if err != nil {
		return fmt.Errorf("attaching session: %w", err)
	}
	return nil
}

func (s Session) sessionName() string {
	return s.path.SessionName
}
