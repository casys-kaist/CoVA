class LabelHelper:
    def __init__(self, filepath):
        self.labels = []
        with open(filepath) as f:
            for line in f:
                self.labels.append(line.strip())

    def label_to_num(self, label):
        return self.labels.index(label)

    def num_to_label(self, num):
        return self.labels[num]
